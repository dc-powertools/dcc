use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::{
    cache::{CacheDir, StateMount},
    config::{
        self,
        vars::{CONTAINER_CACHE, CONTAINER_WORKSPACE},
        BuildConfig,
    },
    docker, dry_run,
    features::FeatureRuntimeConfig,
    lifecycle::{self, LifecycleCommand, LifecycleHooks},
    profile::{ContainerId, ProfileName},
    supervisor,
    workspace::Workspace,
};

pub(crate) async fn build(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    opts: BuildOptions,
) -> anyhow::Result<()> {
    let cache_dir = CacheDir::new(workspace, profile);

    let config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
        .with_context(|| format!("failed to load config `{}`", config_path.display()))?;
    ensure_devcontainer_unsafe_runtime_allowed(&config, opts.allow_unsafe_runtime)?;

    let container_id = ContainerId::new(workspace, profile);
    let image_tag = container_id.as_image_tag();

    if opts.debug {
        eprintln!("dcc debug: command `build`");
        eprintln!("dcc debug: profile `{}`", profile.as_str());
        eprintln!("dcc debug: config `{}`", config_path.display());
        eprintln!("dcc debug: image tag `{}`", image_tag.as_str());
        match (config.image.as_deref(), config.build.as_ref()) {
            (Some(image), None) => eprintln!("dcc debug: image source `{image}`"),
            (None, Some(build)) => {
                eprintln!("dcc debug: build context `{}`", build.context);
                eprintln!("dcc debug: build dockerfile `{}`", build.dockerfile);
                if let Some(target) = &build.target {
                    eprintln!("dcc debug: build target `{target}`");
                }
            }
            _ => {}
        }
        eprintln!("dcc debug: refresh-only `{}`", opts.refresh_only);
        let remap = crate::uid::plan_uid_remap(
            &config.container_user,
            config.update_remote_user_uid,
            crate::uid::host_ids(),
        );
        match &remap {
            crate::uid::RemapPlan::Remap { uid, gid, user } => eprintln!(
                "dcc debug: updateRemoteUserUID remap user `{user}` to uid {uid} gid {gid}"
            ),
            crate::uid::RemapPlan::None { reason } => {
                eprintln!("dcc debug: updateRemoteUserUID remap skipped ({reason})")
            }
        }
    }

    if opts.dry_run {
        let mut skipped: Vec<String> = vec![
            "docker image build",
            "docker image label inspection",
            "build-preparation container start",
            "build-preparation lifecycle hooks",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        if opts.refresh_only {
            skipped.push("profile image existence check".to_string());
        }
        let mut checks: Vec<String> = vec![
            "workspace resolved",
            "profile resolved",
            "config loaded",
            "devcontainer unsafe runtime checked",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        // Dry-run seeding reporting: planned seeding is knowable from the
        // declared project state without Docker. Feature-contributed state
        // requires the image label and is reported as skipped in dry-run.
        let has_project_state = !config.state.is_empty();
        if has_project_state {
            checks.push("state seeding planned from declared customizations.dcc.state".to_string());
            if opts.reseed_state {
                checks.push(
                    "reseed-state: host state would be overwritten where digests differ"
                        .to_string(),
                );
            }
        } else {
            checks.push("no declared state: no hydration container run planned".to_string());
        }
        // Dry-run remap reporting: the plan is knowable from the config and
        // the host uid/gid without Docker.
        let dry_remap = crate::uid::plan_uid_remap(
            &config.container_user,
            config.update_remote_user_uid,
            crate::uid::host_ids(),
        );
        match &dry_remap {
            crate::uid::RemapPlan::Remap { uid, gid, user } => checks.push(format!(
                "updateRemoteUserUID remap planned: user `{user}` -> uid {uid} gid {gid}"
            )),
            crate::uid::RemapPlan::None { reason } => {
                checks.push(format!("updateRemoteUserUID remap skipped ({reason})"))
            }
        }
        skipped.push("dcc.seed label inspection (Feature state)".to_string());
        skipped.push("state hydration container run".to_string());
        return dry_run::DryRunReport::new(
            if opts.refresh_only {
                "build --refresh-only"
            } else {
                "build"
            },
            workspace,
            profile,
            config_path,
            checks,
            skipped,
        )
        .print(opts.format);
    }

    if opts.refresh_only {
        ensure_refresh_image_exists(image_tag.as_str()).await?;
    } else {
        let base_image = build_base_image(&config, config_path, image_tag.as_str(), opts.no_cache)
            .await
            .context("failed to build base image")?;
        build_dcc_stage(
            &config,
            config_path,
            &base_image,
            image_tag.as_str(),
            opts.no_cache,
            opts.allow_unsafe_runtime,
        )
        .await?;
    }

    run_build_preparation(
        workspace,
        profile,
        config_path,
        &config,
        BuildPrepOptions {
            refresh_only: opts.refresh_only,
            allow_unsafe_runtime: opts.allow_unsafe_runtime,
            reseed_state: opts.reseed_state,
            dry_run: opts.dry_run,
            format: opts.format,
        },
    )
    .await
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuildOptions {
    pub(crate) no_cache: bool,
    pub(crate) refresh_only: bool,
    pub(crate) strict: bool,
    pub(crate) allow_unsafe_runtime: bool,
    pub(crate) reseed_state: bool,
    pub(crate) dry_run: bool,
    pub(crate) debug: bool,
    pub(crate) format: crate::cli::OutputFormat,
}

async fn ensure_refresh_image_exists(image: &str) -> anyhow::Result<()> {
    if docker::image_exists(image).await? {
        return Ok(());
    }
    anyhow::bail!(
        "`dcc build --refresh-only` requires profile image `{image}` to already exist; run `dcc build` first"
    )
}

async fn build_base_image(
    config: &config::DevcontainerConfig,
    config_path: &Path,
    image_tag: &str,
    no_cache: bool,
) -> anyhow::Result<String> {
    let Some(build) = &config.build else {
        return config
            .image
            .clone()
            .context("non-fast build requires an image or build source");
    };

    let plan = plan_official_build(build, config_path)?;
    let base_tag = generated_base_tag(image_tag);
    docker::build_path(docker::DockerBuildOptions {
        tag: base_tag.clone(),
        no_cache,
        pull: no_cache,
        metadata_label: None,
        seed_label: None,
        file: Some(plan.dockerfile),
        context_dir: Some(plan.context_dir),
        build_args: plan.build_args,
        target: build.target.clone(),
    })
    .await
    .with_context(|| format!("failed to build image `{base_tag}` from `build` source"))?;
    Ok(base_tag)
}

async fn build_dcc_stage(
    config: &config::DevcontainerConfig,
    config_path: &Path,
    base_image: &str,
    image_tag: &str,
    no_cache: bool,
    allow_unsafe_runtime: bool,
) -> anyhow::Result<()> {
    let config_dir = config_path.parent().with_context(|| {
        format!(
            "config path `{}` has no parent directory",
            config_path.display()
        )
    })?;
    // Plan the updateRemoteUserUID remap from the host's uid/gid. The remap is
    // baked into the generated build stage; its --build-arg values are passed
    // to `docker build` below. See `src/uid.rs`.
    let remap = crate::uid::plan_uid_remap(
        &config.container_user,
        config.update_remote_user_uid,
        crate::uid::host_ids(),
    );
    let build_args = match &remap {
        crate::uid::RemapPlan::Remap { uid, gid, user } => {
            crate::uid::remap_build_args(*uid, *gid, user)
        }
        crate::uid::RemapPlan::None { .. } => Vec::new(),
    };
    let output = if config.image.as_deref() == Some(base_image) {
        crate::features::build_context(config, config_dir, allow_unsafe_runtime, Some(&remap)).await
    } else {
        crate::features::build_context_from_base_image(
            config,
            base_image,
            config_dir,
            allow_unsafe_runtime,
            Some(&remap),
        )
        .await
    }
    .context("failed to build feature context")?;

    // Build the dcc.seed manifest from the merged Feature + project state.
    // Paths may still contain ${containerEnv:...}; they are resolved after the
    // image exists, at hydration time, the same way build-prep resolves them.
    let merged_state: Vec<config::StateEntry> = output
        .feature_state
        .iter()
        .cloned()
        .chain(config.state.iter().cloned())
        .collect();
    let seed_label = if merged_state.is_empty() {
        None
    } else {
        let manifest = crate::seed::manifest_from_state(&merged_state, image_tag);
        Some(
            manifest
                .to_label()
                .context("failed to serialize dcc.seed label")?,
        )
    };

    let pull_base_image = config.image.as_deref() == Some(base_image);
    docker::build(
        image_tag,
        no_cache,
        no_cache && pull_base_image,
        output.context_tar,
        output.metadata_label.as_deref(),
        seed_label.as_deref(),
        build_args,
    )
    .await
    .with_context(|| format!("failed to build image `{image_tag}`"))?;

    Ok(())
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct OfficialBuildPlan {
    pub(crate) context_dir: PathBuf,
    pub(crate) dockerfile: PathBuf,
    pub(crate) build_args: Vec<(String, String)>,
}

pub(crate) fn plan_official_build(
    build: &BuildConfig,
    config_path: &Path,
) -> anyhow::Result<OfficialBuildPlan> {
    let config_dir = config_path.parent().with_context(|| {
        format!(
            "config path `{}` has no parent directory",
            config_path.display()
        )
    })?;
    let mut build_args: Vec<(String, String)> = build
        .args
        .iter()
        .map(|(key, value)| (key.clone(), value.as_build_arg()))
        .collect();
    build_args.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(OfficialBuildPlan {
        context_dir: config_dir.join(&build.context),
        dockerfile: config_dir.join(&build.dockerfile),
        build_args,
    })
}

fn generated_base_tag(image_tag: &str) -> String {
    format!("{image_tag}-base")
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BuildPrepOptions {
    pub(crate) refresh_only: bool,
    pub(crate) allow_unsafe_runtime: bool,
    pub(crate) reseed_state: bool,
    pub(crate) dry_run: bool,
    pub(crate) format: crate::cli::OutputFormat,
}

async fn run_build_preparation(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    config: &config::DevcontainerConfig,
    opts: BuildPrepOptions,
) -> anyhow::Result<()> {
    let refresh_only = opts.refresh_only;
    let allow_unsafe_runtime = opts.allow_unsafe_runtime;
    let reseed_state = opts.reseed_state;
    let _ = opts.dry_run;
    let _ = opts.format;
    let container_id = ContainerId::new(workspace, profile);
    let image_tag = container_id.as_image_tag();
    let feature_runtime = read_feature_runtime(image_tag.as_str()).await?;
    ensure_unsafe_runtime_allowed(&feature_runtime, allow_unsafe_runtime)?;

    let plan = BuildPrepPlan::new(config, &feature_runtime, refresh_only);

    let cache_dir = CacheDir::new(workspace, profile);
    cache_dir.ensure_exists()?;
    let mut container_env = docker::inspect_image_env(image_tag.as_str())
        .await
        .with_context(|| format!("failed to inspect image env `{image_tag}`"))?;
    // containerEnv probing is needed for state resolution (e.g.
    // ${containerEnv:HOME}) even when there are no build-prep hooks, because
    // hydration resolves state paths the same way.
    if plan.references_container_env(config, &feature_runtime)
        || config
            .state
            .iter()
            .any(|e| e.path.contains("${containerEnv:"))
        || feature_runtime
            .state
            .iter()
            .any(|e| e.path.contains("${containerEnv:"))
    {
        match docker::probe_user_env(image_tag.as_str(), &config.container_user).await {
            Ok(probed) => container_env.extend(probed),
            Err(e) => eprintln!(
                "warning: could not probe container HOME/USER ({e:#}); \
                 ${{containerEnv:HOME}}/${{containerEnv:USER}} may be unresolved"
            ),
        }
    }
    let workdir = config::vars::resolve_container_env(&config.workspace_folder, &container_env);

    let state = resolve_runtime_state(config, &feature_runtime, &container_env)
        .context("invalid customizations.dcc.state after resolving containerEnv")?;

    // Hydrate declared state from the image before preparing the (empty) bind
    // sources, so build-prep hooks observe install-time content. Skipped
    // entirely when there is no declared state.
    if !state.is_empty() {
        hydrate_state(
            &cache_dir,
            image_tag.as_str(),
            &state,
            &config.container_user,
            reseed_state,
        )
        .await
        .context("failed to seed declared state from image")?;
    }

    let state_mounts = cache_dir.plan_state_mounts(&state);
    cache_dir.prepare_state_mounts(&state_mounts)?;

    if plan.hooks.is_empty() {
        return Ok(());
    }

    let rt_dir = supervisor::RtDir::new(workspace, profile);
    rt_dir.materialize()?;
    let rt_mount = rt_dir.mount_arg();

    let local_workspace = workspace.root.to_string_lossy().into_owned();
    let local_cache = cache_dir.host_path.to_string_lossy().into_owned();
    let container_name = build_prep_container_name(container_id.as_str());
    let state_mount_args: Vec<String> = state_mounts.iter().map(StateMount::to_mount_arg).collect();
    let args = build_prep_container_args(BuildPrepContainerArgs {
        container_name: &container_name,
        container_id: container_id.as_str(),
        image: image_tag.as_str(),
        workspace: &local_workspace,
        cache: &local_cache,
        config_path,
        state_mounts: &state_mount_args,
        rt_mount: &rt_mount,
        user: &config.container_user,
        workdir: &workdir,
    });

    docker::start_detached(&args).await.with_context(|| {
        format!("failed to start build-preparation container `{container_name}`")
    })?;
    let hook_result = async {
        wait_for_running(&container_name).await?;
        run_planned_hooks(
            &plan,
            &container_name,
            config,
            &local_workspace,
            &local_cache,
            &container_env,
            &workdir,
        )
        .await
    }
    .await;
    let stop_result = docker::stop_container(&container_name).await;
    hook_result?;
    stop_result
        .with_context(|| format!("failed to stop build-preparation container `{container_name}`"))
}

/// Hydrates declared state from the image: builds the seed manifest from the
/// already-resolved state paths, compares against the host-side ledger and
/// current host digests, runs a one-shot container to copy image content where
/// needed, then updates the ledger.
///
/// The `dcc.seed` image label carries deferred (unresolved) paths for
/// cross-machine runtime use; here the paths are already resolved against this
/// image's env, so the manifest is reconstructed from `state`. The `build_id`
/// is the image tag, matching what the label stores.
async fn hydrate_state(
    cache_dir: &CacheDir,
    image: &str,
    state: &[config::StateEntry],
    container_user: &str,
    reseed_state: bool,
) -> anyhow::Result<()> {
    let manifest = crate::seed::manifest_from_state(state, image);
    if manifest.is_empty() {
        return Ok(());
    }

    let ledger_path = crate::seed::SeedLedger::path(cache_dir);
    let ledger = crate::seed::SeedLedger::read(&ledger_path)?;

    // Compute current host digests for each manifest entry.
    let mut host_digests: HashMap<String, Option<String>> = HashMap::new();
    for entry in &manifest.entries {
        let host_path = crate::seed::state_host_path(cache_dir, &entry.path);
        let digest = crate::seed::host_state_digest(&host_path)
            .with_context(|| format!("failed to digest state path `{}`", entry.path))?;
        host_digests.insert(entry.path.clone(), digest);
    }

    let plan = crate::seed::plan_hydration(&manifest, &ledger, &host_digests, reseed_state);

    // Warn about preserved (user-modified) entries so the user can act.
    for decision in &plan {
        if let crate::seed::HydrationDecision::Preserve { path, .. } = decision {
            let recorded = ledger
                .get(path)
                .and_then(|r| r.seed_digest.clone())
                .unwrap_or_else(|| "(none)".to_string());
            eprintln!(
                "warning: customizations.dcc.state path `{path}` has been modified \
                 (seed digest {recorded}, current host content differs); preserving your data. \
                 To overwrite with the image seed, rerun `dcc build --reseed-state`."
            );
        }
    }

    if !crate::seed::needs_hydration(&plan) {
        return Ok(());
    }

    // Only hydrate entries that need it (Seed or Refresh).
    let to_hydrate: Vec<crate::seed::SeedManifestEntry> = plan
        .iter()
        .filter_map(|d| match d {
            crate::seed::HydrationDecision::Seed { path, kind }
            | crate::seed::HydrationDecision::Refresh { path, kind } => {
                manifest.entries.iter().find(|e| e.path == *path).map(|e| {
                    crate::seed::SeedManifestEntry {
                        path: e.path.clone(),
                        kind: *kind,
                        digest: e.digest.clone(),
                    }
                })
            }
            _ => None,
        })
        .collect();

    let state_root = crate::seed::state_root(cache_dir);
    std::fs::create_dir_all(&state_root)
        .with_context(|| format!("failed to create state root `{}`", state_root.display()))?;
    let state_root_str = state_root.to_string_lossy().into_owned();
    let owner = (container_user != "root").then_some(container_user);
    let args = crate::seed::hydration_container_args(image, &state_root_str, &to_hydrate, owner);
    eprintln!(
        "dcc: seeding {} declared state path(s) from image `{image}`",
        to_hydrate.len()
    );
    docker::run_to_completion(&args)
        .await
        .with_context(|| format!("failed to run hydration container for image `{image}`"))?;

    // Recompute digests of what we just wrote and update the ledger.
    let mut new_entries: Vec<crate::seed::LedgerEntry> = Vec::new();
    for entry in &manifest.entries {
        let host_path = crate::seed::state_host_path(cache_dir, &entry.path);
        let digest = crate::seed::host_state_digest(&host_path)
            .with_context(|| format!("failed to digest seeded path `{}`", entry.path))?;
        new_entries.push(crate::seed::LedgerEntry {
            path: entry.path.clone(),
            kind: entry.kind,
            seed_digest: digest,
            build_id: manifest.build_id.clone(),
        });
    }
    let new_ledger = crate::seed::SeedLedger {
        entries: new_entries,
    };
    new_ledger
        .write(&ledger_path)
        .context("failed to write seed ledger after hydration")?;
    Ok(())
}

async fn read_feature_runtime(image: &str) -> anyhow::Result<FeatureRuntimeConfig> {
    match docker::inspect_image_label(image)
        .await
        .with_context(|| format!("failed to inspect image `{image}`"))?
    {
        None => Ok(FeatureRuntimeConfig::default()),
        Some(json) => crate::features::parse_runtime_from_label(&json).with_context(|| {
            format!("failed to parse devcontainer.metadata label from image `{image}`")
        }),
    }
}

fn resolve_runtime_state(
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    container_env: &HashMap<String, String>,
) -> anyhow::Result<Vec<config::StateEntry>> {
    let state: Vec<config::StateEntry> = feature_runtime
        .state
        .iter()
        .cloned()
        .chain(config.state.iter().cloned())
        .collect();
    config::resolve::resolve_state_entries_container_env(&state, container_env)
}

fn build_prep_container_name(container_id: &str) -> String {
    format!("{container_id}-build-prep")
}

struct BuildPrepContainerArgs<'a> {
    container_name: &'a str,
    container_id: &'a str,
    image: &'a str,
    workspace: &'a str,
    cache: &'a str,
    config_path: &'a Path,
    state_mounts: &'a [String],
    rt_mount: &'a str,
    user: &'a str,
    workdir: &'a str,
}

fn build_prep_container_args(input: BuildPrepContainerArgs<'_>) -> Vec<String> {
    let mut args = Vec::new();
    args.extend(["--name".to_string(), input.container_name.to_string()]);
    args.extend([
        "--label".to_string(),
        format!("dcc.container_id={}", input.container_id),
    ]);
    args.extend([
        "--label".to_string(),
        format!("devcontainer.local_folder={}", input.workspace),
    ]);
    args.extend([
        "--label".to_string(),
        format!("devcontainer.config_file={}", input.config_path.display()),
    ]);
    args.push("--rm".to_string());
    args.push("-dit".to_string());
    args.extend(["--workdir".to_string(), input.workdir.to_string()]);
    args.extend(["-u".to_string(), input.user.to_string()]);
    args.push("-v".to_string());
    args.push(format!("{}:{CONTAINER_WORKSPACE}", input.workspace));
    args.push("-v".to_string());
    args.push(format!("{}:{CONTAINER_CACHE}", input.cache));
    for mount in input.state_mounts {
        args.extend(["--mount".to_string(), mount.clone()]);
    }
    args.push("--mount".to_string());
    args.push(input.rt_mount.to_string());
    args.extend(["--tmpfs".to_string(), format!("{CONTAINER_WORKSPACE}/.dcc")]);
    args.extend([
        "--tmpfs".to_string(),
        format!("{}:mode=1777", supervisor::STATE_DIR),
    ]);
    // Durable mode: the build-prep container is always host-stopped, so the
    // supervisor never drain-exits during hook execution.
    args.extend([
        "--entrypoint".to_string(),
        format!("{}/dcc-supervisor", supervisor::DCC_SHARE),
    ]);
    // Image tag must come after all Docker flags; supervisor entrypoint
    // arguments follow the image tag. Build-prep hooks run host-side, so there
    // is no --expect-command or --start-hooks.
    supervisor::append_run_image_and_args(&mut args, input.image, "durable", false, None);
    args
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BuildPrepHook {
    phase: &'static str,
    source: String,
    command: LifecycleCommand,
}

struct BuildPrepPlan {
    hooks: Vec<BuildPrepHook>,
}

impl BuildPrepPlan {
    fn new(
        config: &config::DevcontainerConfig,
        feature_runtime: &FeatureRuntimeConfig,
        refresh_only: bool,
    ) -> Self {
        Self {
            hooks: collect_build_prep_hooks(config, feature_runtime, refresh_only),
        }
    }

    fn references_container_env(
        &self,
        config: &config::DevcontainerConfig,
        feature_runtime: &FeatureRuntimeConfig,
    ) -> bool {
        const NEEDLE: &str = "${containerEnv:";
        self.hooks
            .iter()
            .any(|hook| describe_lifecycle_command(&hook.command).contains(NEEDLE))
            || config.workspace_folder.contains(NEEDLE)
            || config.state.iter().any(|entry| entry.path.contains(NEEDLE))
            || feature_runtime
                .state
                .iter()
                .any(|entry| entry.path.contains(NEEDLE))
    }
}

fn collect_build_prep_hooks(
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    refresh_only: bool,
) -> Vec<BuildPrepHook> {
    type BuildPrepHookAccessor = fn(&LifecycleHooks) -> &Option<LifecycleCommand>;
    let phases: [(&str, BuildPrepHookAccessor); 3] = [
        ("onCreateCommand", |hooks: &LifecycleHooks| {
            &hooks.on_create_command
        }),
        ("updateContentCommand", |hooks: &LifecycleHooks| {
            &hooks.update_content_command
        }),
        ("postCreateCommand", |hooks: &LifecycleHooks| {
            &hooks.post_create_command
        }),
    ];
    let mut planned = Vec::new();
    for (phase, get) in phases {
        if refresh_only && phase == "onCreateCommand" {
            continue;
        }
        for (feature_id, hooks) in &feature_runtime.feature_hooks {
            if let Some(command) = get(hooks) {
                planned.push(BuildPrepHook {
                    phase,
                    source: format!("feature `{feature_id}`"),
                    command: command.clone(),
                });
            }
        }
        if let Some(command) = get(&config.lifecycle) {
            planned.push(BuildPrepHook {
                phase,
                source: "project".to_string(),
                command: command.clone(),
            });
        }
    }
    planned
}

async fn run_planned_hooks(
    plan: &BuildPrepPlan,
    container: &str,
    config: &config::DevcontainerConfig,
    local_workspace: &str,
    local_cache: &str,
    container_env: &HashMap<String, String>,
    workdir: &str,
) -> anyhow::Result<()> {
    let substitute = |s: &str| -> anyhow::Result<String> {
        let s = config::vars::apply_substitution(s, local_workspace, local_cache);
        Ok(config::vars::resolve_container_env(&s, container_env))
    };
    for hook in &plan.hooks {
        let cmd = hook.command.try_substitute(&substitute).with_context(|| {
            format!(
                "{} from {} contains an invalid variable",
                hook.phase, hook.source
            )
        })?;
        lifecycle::run_in_container(&cmd, container, &config.container_user, workdir)
            .await
            .with_context(|| format!("{} from {} failed", hook.phase, hook.source))?;
    }
    Ok(())
}

async fn wait_for_running(container: &str) -> anyhow::Result<()> {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
    const POLL: std::time::Duration = std::time::Duration::from_millis(100);
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        if docker::inspect_running(container).await? {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("timed out after 10 s waiting for container to start");
        }
        tokio::time::sleep(POLL).await;
    }
}

fn ensure_unsafe_runtime_allowed(
    feature_runtime: &FeatureRuntimeConfig,
    allow_unsafe_runtime: bool,
) -> anyhow::Result<()> {
    if allow_unsafe_runtime || feature_runtime.unsafe_runtime.is_empty() {
        return Ok(());
    }
    anyhow::bail!(
        "image metadata contains unsafe Feature runtime setting(s); rerun with `--allow-unsafe-runtime` to allow them"
    )
}

fn ensure_devcontainer_unsafe_runtime_allowed(
    config: &config::DevcontainerConfig,
    allow_unsafe_runtime: bool,
) -> anyhow::Result<()> {
    if allow_unsafe_runtime || config.unsafe_runtime.is_empty() {
        return Ok(());
    }
    anyhow::bail!(
        "devcontainer config contains unsafe runtime setting(s) {}; rerun with `--allow-unsafe-runtime` to allow them",
        config.unsafe_runtime.property_names().join(", ")
    )
}

fn describe_lifecycle_command(cmd: &LifecycleCommand) -> String {
    use lifecycle::{LifecycleCommand as C, LifecycleCommandSingle as S};
    let single = |s: &S| match s {
        S::Shell(sh) => sh.clone(),
        S::Exec(argv) => argv.join(" "),
    };
    match cmd {
        C::Shell(s) => s.clone(),
        C::Exec(argv) => argv.join(" "),
        C::Parallel(map) => map
            .iter()
            .map(|(k, v)| format!("{k}: {}", single(v)))
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BuildArgValue, StateEntry, StateKind};
    use indexmap::IndexMap;

    fn config() -> config::DevcontainerConfig {
        config::DevcontainerConfig {
            name: None,
            image: Some("rust:1".to_string()),
            build: None,
            features: IndexMap::new(),
            container_env: HashMap::new(),
            remote_env: HashMap::new(),
            container_user: "root".to_string(),
            mounts: Vec::new(),
            run_args: Vec::new(),
            unsafe_runtime: config::UnsafeRuntimeConfig::default(),
            forward_ports: Vec::new(),
            ports_attributes: HashMap::new(),
            other_ports_attributes: None,
            override_command: None,
            update_remote_user_uid: true,
            workspace_folder: CONTAINER_WORKSPACE.to_string(),
            workspace_mount: None,
            initialize_command: None,
            lifecycle: LifecycleHooks::default(),
            scripts: HashMap::new(),
            state: Vec::new(),
        }
    }

    fn shell(s: &str) -> Option<LifecycleCommand> {
        Some(LifecycleCommand::Shell(s.to_string()))
    }

    #[test]
    fn plan_official_build_resolves_paths_and_args() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join(".devcontainer/devcontainer.json");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let mut args = HashMap::new();
        args.insert("ZED".to_string(), BuildArgValue::String("last".to_string()));
        args.insert("ALPHA".to_string(), BuildArgValue::Bool(true));
        let build = BuildConfig {
            context: "..".to_string(),
            dockerfile: "Dockerfile".to_string(),
            args,
            target: Some("dev".to_string()),
        };
        let plan = plan_official_build(&build, &config_path).unwrap();
        assert_eq!(plan.context_dir, tmp.path().join(".devcontainer/.."));
        assert_eq!(plan.dockerfile, tmp.path().join(".devcontainer/Dockerfile"));
        assert_eq!(
            plan.build_args,
            vec![
                ("ALPHA".to_string(), "true".to_string()),
                ("ZED".to_string(), "last".to_string()),
            ]
        );
    }

    #[test]
    fn collect_build_prep_hooks_orders_feature_then_project_by_phase() {
        let mut config = config();
        config.lifecycle.on_create_command = shell("project create");
        config.lifecycle.update_content_command = shell("project update");
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.feature_hooks.push((
            "feat-a".to_string(),
            LifecycleHooks {
                on_create_command: shell("feature create"),
                post_create_command: shell("feature post"),
                ..Default::default()
            },
        ));

        let hooks = collect_build_prep_hooks(&config, &runtime, false);
        let summary: Vec<(&str, &str)> = hooks
            .iter()
            .map(|hook| (hook.phase, hook.source.as_str()))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("onCreateCommand", "feature `feat-a`"),
                ("onCreateCommand", "project"),
                ("updateContentCommand", "project"),
                ("postCreateCommand", "feature `feat-a`"),
            ]
        );
    }

    #[test]
    fn refresh_only_skips_on_create() {
        let mut config = config();
        config.lifecycle.on_create_command = shell("project create");
        config.lifecycle.update_content_command = shell("project update");
        let hooks = collect_build_prep_hooks(&config, &FeatureRuntimeConfig::default(), true);
        assert_eq!(
            hooks.iter().map(|hook| hook.phase).collect::<Vec<_>>(),
            vec!["updateContentCommand"]
        );
    }

    #[test]
    fn build_prep_container_args_include_state_mounts_without_reset_flags() {
        let args = build_prep_container_args(BuildPrepContainerArgs {
            container_name: "dcc-id-build-prep",
            container_id: "dcc-id",
            image: "dcc-id",
            workspace: "/workspace",
            cache: "/workspace/.dcc/dev",
            config_path: Path::new("/workspace/.devcontainer/devcontainer.json"),
            state_mounts: &[
                "type=bind,src=/workspace/.dcc/dev/state/home/dev/.cargo,dst=/home/dev/.cargo"
                    .to_string(),
            ],
            rt_mount:
                "type=bind,source=/workspace/.dcc/dev.rt,target=/usr/local/share/dcc/rt,readonly",
            user: "dev",
            workdir: "/workspace/service",
        });
        let workdir = args
            .windows(2)
            .find(|pair| pair[0] == "--workdir")
            .map(|pair| pair[1].as_str());
        assert_eq!(workdir, Some("/workspace/service"));
        assert!(args.contains(&"--mount".to_string()));
        assert!(args.contains(
            &"type=bind,src=/workspace/.dcc/dev/state/home/dev/.cargo,dst=/home/dev/.cargo"
                .to_string()
        ));
        let image_pos = args.iter().position(|arg| arg == "dcc-id").unwrap();
        let mode_pos = args.iter().position(|arg| arg == "--mode").unwrap();
        assert!(
            image_pos < mode_pos,
            "supervisor entrypoint args must come after the image: {args:?}"
        );
        assert!(!args.contains(&"--no-cache".to_string()));
    }

    #[test]
    fn resolve_runtime_state_merges_feature_before_project() {
        let mut config = config();
        config.state.push(StateEntry {
            path: "/workspace/target".to_string(),
            kind: StateKind::Directory,
        });
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.state.push(StateEntry {
            path: "/home/dev/.cargo".to_string(),
            kind: StateKind::Directory,
        });
        let state = resolve_runtime_state(&config, &runtime, &HashMap::new()).unwrap();
        assert_eq!(
            state,
            vec![
                StateEntry {
                    path: "/home/dev/.cargo".to_string(),
                    kind: StateKind::Directory,
                },
                StateEntry {
                    path: "/workspace/target".to_string(),
                    kind: StateKind::Directory,
                },
            ]
        );
    }
}
