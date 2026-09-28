use std::io::IsTerminal as _;
use std::path::Path;
use std::process::ExitStatus;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config::{
        self,
        vars::{CONTAINER_CACHE, CONTAINER_WORKSPACE},
    },
    docker, dry_run,
    features::{self, FeatureRuntimeConfig, FeatureUnsafeRuntime},
    forward, lifecycle,
    profile::{ContainerId, ContainerName, ProfileName},
    supervisor, version,
    workspace::Workspace,
};

/// CPU and memory limits forwarded to `docker run`.
#[derive(Clone, Copy)]
pub(crate) struct ResourceLimits<'a> {
    pub(crate) memory: &'a str,
    pub(crate) cpus: &'a str,
    pub(crate) memory_explicit: bool,
    pub(crate) cpus_explicit: bool,
}

/// Behavioral options for a container launch, shared by `dcc exec` and `dcc run`.
#[derive(Clone, Copy)]
pub(crate) struct ExecOptions<'a> {
    pub(crate) limits: ResourceLimits<'a>,
    pub(crate) skip_lifecycle: bool,
    pub(crate) debug: bool,
    pub(crate) strict: bool,
    pub(crate) profile_arg: &'a str,
    pub(crate) allow_unsafe_runtime: bool,
    pub(crate) keep: bool,
    pub(crate) dry_run: bool,
    pub(crate) format: crate::cli::OutputFormat,
}

pub(crate) async fn exec(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    override_args: &[String],
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
    if opts.dry_run {
        dry_run_runtime(
            workspace,
            profile,
            config_path,
            "exec",
            override_args,
            opts,
            vec![
                "docker image/container inspection",
                "docker run",
                "docker exec",
            ],
        )?;
        return Ok(success_status());
    }
    execute_foreground(
        workspace,
        profile,
        config_path,
        override_args,
        ForegroundKind::Exec,
        opts,
    )
    .await
}

pub(crate) async fn attach(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    override_args: &[String],
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
    let raw_args = if override_args.is_empty() {
        default_attach_command()
    } else {
        override_args.to_vec()
    };
    if opts.dry_run {
        dry_run_runtime(
            workspace,
            profile,
            config_path,
            "attach",
            &raw_args,
            opts,
            vec![
                "docker image/container inspection",
                "docker run",
                "postAttachCommand hooks",
                "docker exec",
            ],
        )?;
        return Ok(success_status());
    }
    execute_foreground(
        workspace,
        profile,
        config_path,
        &raw_args,
        ForegroundKind::Attach,
        opts,
    )
    .await
}

pub(crate) async fn start(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    opts: ExecOptions<'_>,
) -> anyhow::Result<()> {
    if opts.dry_run {
        dry_run_runtime(
            workspace,
            profile,
            config_path,
            "start",
            &[],
            opts,
            vec![
                "docker image/container inspection",
                "docker run",
                "postStartCommand hooks",
            ],
        )?;
        return Ok(());
    }
    acquire_runtime(workspace, profile, config_path, &[], None, opts).await?;
    Ok(())
}

pub(crate) fn dry_run_runtime(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    command: &str,
    override_args: &[String],
    opts: ExecOptions<'_>,
    mut skipped: Vec<&'static str>,
) -> anyhow::Result<()> {
    let cache_dir = CacheDir::new(workspace, profile);
    let config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
        .with_context(|| format!("failed to load config `{}`", config_path.display()))?;
    let empty_feature_runtime = FeatureRuntimeConfig::default();
    ensure_unsafe_runtime_allowed(&config, &empty_feature_runtime, opts.allow_unsafe_runtime)?;
    sanitize_run_args(&config.run_args, opts.allow_unsafe_runtime)?;
    ensure_mounts_safe(&config.mounts, opts.allow_unsafe_runtime)?;
    if references_container_env(override_args, &config, &empty_feature_runtime) {
        skipped.push("containerEnv resolution from image/user probe");
    }
    skipped.extend([
        "devcontainer.metadata feature runtime inspection",
        "state/cache mount preparation",
        "port forwarding",
        "running snapshot lookup and configuration drift comparison",
    ]);
    dry_run::DryRunReport::new(
        command,
        workspace,
        profile,
        config_path,
        vec![
            "workspace resolved",
            "profile resolved",
            "config loaded",
            "devcontainer unsafe runtime checked",
            "runArgs checked",
            "mount safety checked",
        ],
        skipped,
    )
    .print(opts.format)
}

#[cfg(unix)]
fn success_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt as _;
    ExitStatus::from_raw(0)
}

#[cfg(windows)]
fn success_status() -> ExitStatus {
    use std::os::windows::process::ExitStatusExt as _;
    ExitStatus::from_raw(0)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ForegroundKind {
    Exec,
    Attach,
}

async fn acquire_runtime(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    override_args: &[String],
    named: Option<&str>,
    opts: ExecOptions<'_>,
) -> anyhow::Result<crate::runtime::Runtime> {
    if let Some(runtime) = crate::runtime::reuse(workspace, profile, config_path, opts).await? {
        return Ok(runtime);
    }
    let plan = RuntimePlan::prepare(workspace, profile, config_path, override_args, opts).await?;
    // Reject invalid named commands before starting a container or running hooks.
    if let Some(name) = named {
        crate::run::resolve_script(name, &plan.snapshot.scripts, &plan.snapshot.feature_scripts)?;
    }
    match crate::runtime::create(&plan.run_args, plan.container.as_str(), &plan.snapshot).await? {
        crate::runtime::Created::Winner(container) => {
            crate::runtime::load(container, workspace, profile, config_path, opts).await
        }
        crate::runtime::Created::New(container) => {
            let ready = docker::control(&container.id, "wait-ready", None).await?;
            docker::captured_ok(
                &ready,
                "container startup (retained for diagnosis where possible)",
            )?;
            // Verify the fixed private snapshot can actually be read under Docker's UID mapping.
            let data = docker::control(&container.id, "snapshot", None).await?;
            anyhow::ensure!(data.status.success(), "cannot read runtime snapshot under container UID mapping; stop and recreate with supported permissions");
            let snapshot = crate::runtime_snapshot::RuntimeSnapshot::decode(
                &data.stdout,
                &plan.snapshot.identity,
                &container.image,
                &plan.snapshot.token,
            )?;
            Ok(crate::runtime::Runtime {
                id: container.id,
                snapshot,
                started: true,
            })
        }
    }
}

pub(crate) async fn run_named(
    workspace: &Workspace,
    profile: &ProfileName,
    path: &Path,
    name: &str,
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
    let runtime = acquire_runtime(workspace, profile, path, &[], Some(name), opts).await?;
    let script = crate::run::resolve_script(
        name,
        &runtime.snapshot.scripts,
        &runtime.snapshot.feature_scripts,
    )?;
    let args = vec!["/bin/sh".into(), "-c".into(), script.to_owned()];
    execute_in_runtime(runtime, args, ForegroundKind::Exec, opts).await
}

async fn execute_foreground(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    override_args: &[String],
    kind: ForegroundKind,
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
    let runtime =
        acquire_runtime(workspace, profile, config_path, override_args, None, opts).await?;
    let args = runtime.snapshot.explicit_args(override_args)?;
    execute_in_runtime(runtime, args, kind, opts).await
}

async fn execute_in_runtime(
    runtime: crate::runtime::Runtime,
    args: Vec<String>,
    kind: ForegroundKind,
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
    let snapshot = &runtime.snapshot;
    if kind == ForegroundKind::Attach && opts.skip_lifecycle && !snapshot.attach_hooks.is_empty() {
        eprintln!("warning: skipping postAttachCommand (--skip-lifecycle)");
    }
    if kind == ForegroundKind::Attach && !opts.skip_lifecycle {
        for hook in &snapshot.attach_hooks {
            lifecycle::run_in_container(hook, &runtime.id, &snapshot.user, &snapshot.workdir)
                .await
                .map_err(|_| anyhow::anyhow!("frozen postAttachCommand failed"))?;
        }
    }
    let mut wrapped = vec![format!("{}/dcc-exec", supervisor::DCC_SHARE)];
    wrapped.extend(args);
    let status = docker::exec_foreground(
        &runtime.id,
        &snapshot.user,
        &snapshot.workdir,
        &wrapped,
        std::io::stdin().is_terminal(),
    )
    .await?;
    if should_wait_for_one_shot_teardown(runtime.started, opts.keep) {
        wait_for_no_running_container(&snapshot.identity).await;
    }
    Ok(status)
}

fn should_wait_for_one_shot_teardown(started: bool, keep: bool) -> bool {
    started && !keep
}

/// Wait for Docker to stop reporting any running container for this profile label
/// (e.g. after a one-shot supervisor drains and exits). Best-effort: does not error
/// if the container never disappears.
async fn wait_for_no_running_container(container_id: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if let Ok(None) = docker::running_container_name_by_id(container_id).await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

fn default_attach_command() -> Vec<String> {
    vec![
        "/bin/sh".to_string(),
        "-lc".to_string(),
        "if [ -n \"${SHELL:-}\" ] && [ \"${SHELL#/}\" != \"$SHELL\" ] && [ -x \"$SHELL\" ]; then exec \"$SHELL\"; elif [ -x /bin/bash ]; then exec /bin/bash; else exec /bin/sh; fi".to_string(),
    ]
}

struct RuntimePlan {
    _assets: supervisor::RtDir,
    container: ContainerName,
    run_args: Vec<String>,
    snapshot: crate::runtime_snapshot::RuntimeSnapshot,
}

#[derive(Clone)]
struct OwnedExecOptions {
    limits_memory: String,
    limits_cpus: String,
    skip_lifecycle: bool,
    debug: bool,
    strict: bool,
    profile_arg: String,
    allow_unsafe_runtime: bool,
    keep: bool,
}

impl From<ExecOptions<'_>> for OwnedExecOptions {
    fn from(opts: ExecOptions<'_>) -> Self {
        Self {
            limits_memory: opts.limits.memory.to_string(),
            limits_cpus: opts.limits.cpus.to_string(),
            skip_lifecycle: opts.skip_lifecycle,
            debug: opts.debug,
            strict: opts.strict,
            profile_arg: opts.profile_arg.to_string(),
            allow_unsafe_runtime: opts.allow_unsafe_runtime,
            keep: opts.keep,
        }
    }
}

fn append_runtime_container_labels(args: &mut Vec<String>, container_id: &str) {
    args.extend([
        "--label".to_string(),
        format!("{}={container_id}", docker::CONTAINER_ID_LABEL),
        "--label".to_string(),
        format!(
            "{}={}",
            docker::CONTAINER_ROLE_LABEL,
            docker::CONTAINER_ROLE_RUNTIME
        ),
    ]);
}

impl RuntimePlan {
    async fn prepare(
        workspace: &Workspace,
        profile: &ProfileName,
        config_path: &Path,
        override_args: &[String],
        opts: ExecOptions<'_>,
    ) -> anyhow::Result<Self> {
        let opts = OwnedExecOptions::from(opts);
        let cache_dir = CacheDir::new(workspace, profile);

        let mut config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
            .with_context(|| format!("failed to load config `{}`", config_path.display()))?;

        supervisor::RtDir::prune(workspace, profile).await;
        let (rt_dir, launch_token) = supervisor::RtDir::instance(workspace, profile)?;

        let container_id = ContainerId::new(workspace, profile);
        let container = ContainerName::resolve(config.name.as_deref(), &container_id);
        let image_tag = docker::image_id(container_id.as_image_tag().as_str()).await?;

        // Ensure cache directory exists, then create any cache subdirectories
        // referenced as bind-mount sources (e.g. ${localCacheFolder}/node_modules).
        // Docker requires bind-mount source paths to exist on the host before startup.
        cache_dir.ensure_exists()?;
        rt_dir.materialize()?;

        version::ensure_image_version_compatible(
            image_tag.as_str(),
            &opts.profile_arg,
            opts.strict,
        )
        .await?;

        // Read runtime contributions from the image's devcontainer.metadata label.
        let metadata = docker::inspect_image_label(image_tag.as_str()).await?;
        let feature_runtime = match metadata.as_deref() {
            None => FeatureRuntimeConfig::default(),
            Some(json) => features::parse_runtime_from_label(json).with_context(|| {
                format!("failed to parse devcontainer.metadata label from image `{image_tag}`")
            })?,
        };
        ensure_unsafe_runtime_allowed(&config, &feature_runtime, opts.allow_unsafe_runtime)?;

        let local_workspace = workspace.root.to_string_lossy().into_owned();
        let local_cache = cache_dir.host_path.to_string_lossy().into_owned();

        // The image's baked environment (base image ENV + all containerEnv), used to
        // resolve `${containerEnv:VAR}` references in the runtime properties below.
        // remoteEnv is intentionally absent (it is not part of the image).
        let mut container_env = docker::inspect_image_env(image_tag.as_str())
            .await
            .with_context(|| format!("failed to inspect image env `{image_tag}`"))?;

        // `${containerEnv:HOME}`/`${containerEnv:USER}` are set by the container runtime
        // (from /etc/passwd + the `-u` user), not baked into the image's Config.Env. When
        // any runtime-applied field references `${containerEnv:…}`, probe the configured
        // user's HOME/USER and merge them in. Best-effort: a probe failure warns and
        // leaves them absent, so an unguarded HOME/USER reference fails while an
        // explicit default remains available.
        {
            match docker::probe_user_env(image_tag.as_str(), &config.container_user).await {
                Ok(probed) => container_env.extend(probed),
                Err(e) => eprintln!(
                    "warning: could not probe container HOME/USER ({e:#}); \
                 unguarded ${{containerEnv:HOME}}/${{containerEnv:USER}} references will fail"
                ),
            }
        }

        config.workspace_folder =
            config::vars::resolve_container_env(&config.workspace_folder, &container_env)
                .context("workspaceFolder contains an invalid containerEnv reference")?;
        config.run_args = config
            .run_args
            .iter()
            .enumerate()
            .map(|(index, arg)| {
                config::vars::resolve_container_env(arg, &container_env).with_context(|| {
                    format!("runArgs[{index}] contains an invalid containerEnv reference")
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let state = resolve_runtime_state(&config, &feature_runtime, &container_env)
            .context("invalid customizations.dcc.state after resolving containerEnv")?;

        // Runtime seed guard: when state is declared, check the ledger against the
        // image's dcc.seed label. Warn on build_id mismatch (e.g. cloned repo with a
        // stale .dcc); hydrate only entries with no ledger record at all (the wiped-
        // .dcc recovery case). Content re-digesting is deliberately off the hot path.
        if !state.is_empty() {
            runtime_seed_guard(
                &cache_dir,
                image_tag.as_str(),
                &state,
                &config.container_user,
            )
            .await;
        }

        let state_mounts = cache_dir.plan_state_mounts(&state);
        cache_dir.prepare_state_mounts(&state_mounts)?;
        let state_mount_args: Vec<String> = state_mounts
            .iter()
            .map(|mount| mount.to_mount_arg())
            .collect();

        // The container command (a `dcc run` script or `dcc exec` args) supports the
        // same substitution (`${localEnv:VAR}`, `${containerEnv:VAR}`, …) as
        // mounts/remoteEnv.
        let _validated_args: Vec<String> = override_args
            .iter()
            .enumerate()
            .map(|(index, a)| {
                let a = config::vars::apply_substitution(a, &local_workspace, &local_cache);
                config::vars::resolve_container_env(&a, &container_env)
                    .with_context(|| {
                        format!(
                            "container command argument {index} contains an invalid containerEnv reference"
                        )
                    })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // Mounts: feature contributions first, then devcontainer.json mounts. Feature
        // values get host/localEnv substitution; `${containerEnv:…}` is then resolved
        // over the whole set (devcontainer.json values were host-substituted at load).
        let all_mounts: Vec<String> = feature_runtime
            .mounts
            .iter()
            .map(|m| config::vars::apply_substitution(m, &local_workspace, &local_cache))
            .chain(config.mounts.iter().cloned())
            .enumerate()
            .map(|(index, mount)| {
                config::vars::resolve_container_env(&mount, &container_env).with_context(|| {
                    format!("mount {index} contains an invalid containerEnv reference")
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        ensure_mounts_safe(&all_mounts, opts.allow_unsafe_runtime)?;
        ensure_cache_mount_sources(&all_mounts, &cache_dir)?;
        let safe_run_args = sanitize_run_args(&config.run_args, opts.allow_unsafe_runtime)?;
        let ports = forward::plan_ports(&config.forward_ports, config.relay_port_range)?;
        if !ports.is_empty() {
            crate::runtime::forwarding_supported(&safe_run_args).await?;
        }

        // Combined remoteEnv (devcontainer.json first, then features), fully resolved:
        // feature values get host/localEnv substitution, then `${containerEnv:…}` is
        // resolved over both sources.
        let remote_env: Vec<(String, String)> = config
            .remote_env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .chain(feature_runtime.remote_env.iter().map(|(k, v)| {
                (
                    k.clone(),
                    config::vars::apply_substitution(v, &local_workspace, &local_cache),
                )
            }))
            .map(|(key, value)| {
                let value = config::vars::resolve_container_env(&value, &container_env)
                    .with_context(|| {
                        format!("remoteEnv `{key}` contains an invalid containerEnv reference")
                    })?;
                Ok((key, value))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        // Warn about any ${...} reference still unresolved in a mount or remoteEnv
        // value (e.g. an unsupported ${localEnv:…}); these otherwise make `docker run`
        // fail with an opaque error, so surfacing them here points at the cause.
        for mount in &all_mounts {
            warn_unresolved_variables("mount", mount);
        }
        for (k, v) in &remote_env {
            warn_unresolved_variables(&format!("remoteEnv `{k}`"), v);
        }

        // Build the docker run argument list
        let mut args: Vec<String> = Vec::new();

        args.extend(["--name".into(), container.as_str().to_owned()]);
        append_runtime_container_labels(&mut args, container_id.as_str());
        args.extend(["--label".into(), format!("dcc.launch_token={launch_token}")]);
        args.extend([
            "--label".into(),
            format!("devcontainer.local_folder={}", workspace.root.display()),
        ]);
        args.extend([
            "--label".into(),
            format!("devcontainer.config_file={}", config_path.display()),
        ]);
        args.push("--rm".into());
        args.push("-it".into());
        args.extend(["--workdir".into(), config.workspace_folder.clone()]);
        args.extend(["--memory".into(), opts.limits_memory.clone()]);
        args.extend(["--cpus".into(), opts.limits_cpus.clone()]);
        args.extend(safe_run_args);

        append_unsafe_runtime_args(
            &mut args,
            &config.unsafe_runtime,
            &feature_runtime.unsafe_runtime,
            opts.allow_unsafe_runtime,
        );

        // containerUser (defaults to "dev" when not set in the devcontainer config)
        args.extend(["-u".into(), config.container_user.clone()]);

        // remoteEnv: devcontainer.json + feature, fully substituted (see above).
        for (k, v) in &remote_env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }

        // Initial container mode for the supervisor (PID 1). Passed as an
        // entrypoint argument so the supervisor is born in the right mode; only
        // --keep against an already-running container needs a dcc-ctl mode
        // promotion.
        let mode = supervisor::mode_value(opts.keep);

        // mounts: feature contributions first, then devcontainer.json mounts
        for mount in &all_mounts {
            args.push("--mount".into());
            args.push(mount.clone());
        }

        // workspace bind mount
        args.push("-v".into());
        args.push(format!(
            "{}:{CONTAINER_WORKSPACE}",
            workspace.root.display()
        ));

        // cache bind mount
        args.push("-v".into());
        args.push(format!(
            "{}:{CONTAINER_CACHE}",
            cache_dir.host_path.display()
        ));

        // profile-local state bind mounts
        for mount in &state_mount_args {
            args.push("--mount".into());
            args.push(mount.clone());
        }

        // Read-only bind mount of the startup hook scripts (only `start-hooks/`
        // lives here; the supervisor scripts are baked into the image). Lives
        // outside the /cache mount (a sibling of the cache root) so container-side
        // code can execute but not modify them.
        args.push("--mount".into());
        args.push(rt_dir.mount_arg());

        // mask .dcc directory inside container
        args.extend(["--tmpfs".into(), format!("{CONTAINER_WORKSPACE}/.dcc")]);

        // Container-private lifecycle state for the supervisor. Dies with the container;
        // never host-backed.
        args.extend([
            "--tmpfs".into(),
            format!("{}:mode=1777", supervisor::STATE_DIR),
        ]);

        // PID 1 is the dcc lifecycle supervisor. It owns mode, startup hooks,
        // readiness, the active-command set, and the teardown decision. User
        // commands run via `docker exec` through the dcc-exec wrapper, which
        // registers with the supervisor and waits for readiness before running.
        args.extend([
            "--entrypoint".into(),
            format!("{}/dcc-supervisor", supervisor::DCC_SHARE),
        ]);

        // Emit postStartCommand hook scripts (feature hooks first in installation
        // order, then the devcontainer hook), fully substituted host-side so no
        // ${containerEnv:...} reaches the container. When --skip-lifecycle is set,
        // no scripts are written and the supervisor marks itself ready immediately.
        let has_start_hooks = !opts.skip_lifecycle
            && supervisor::RtDir::has_start_hooks(
                &feature_runtime.feature_hooks,
                &config.lifecycle,
            );
        if opts.skip_lifecycle {
            for warning in
                skipped_hook_warnings(&config, &feature_runtime, RuntimeHookPhase::Startup)
            {
                eprintln!("warning: {warning}");
            }
        }
        if has_start_hooks {
            let substitute = |s: &str| -> anyhow::Result<String> {
                let s = config::vars::apply_substitution(s, &local_workspace, &local_cache);
                config::vars::resolve_container_env(&s, &container_env)
            };
            rt_dir
                .write_start_hooks(
                    &feature_runtime.feature_hooks,
                    &config.lifecycle,
                    &substitute,
                    &config.container_user,
                    &config.workspace_folder,
                )
                .context("failed to write startup hook scripts")?;
        }

        // Image tag must come after all Docker flags; supervisor entrypoint
        // arguments follow the image tag.
        let start_hooks_container_path =
            has_start_hooks.then(|| rt_dir.start_hooks_container_path());
        supervisor::append_run_image_and_args(
            &mut args,
            image_tag.as_str(),
            mode,
            !opts.keep,
            start_hooks_container_path.as_deref(),
        );

        let substitute = |s: &str| {
            let s = config::vars::apply_substitution(s, &local_workspace, &local_cache);
            config::vars::resolve_container_env(&s, &container_env)
        };
        let mut attach_hooks = Vec::new();
        for (_, hooks) in &feature_runtime.feature_hooks {
            if let Some(hook) = &hooks.post_attach_command {
                attach_hooks.push(hook.try_substitute(&substitute)?);
            }
        }
        if let Some(hook) = &config.lifecycle.post_attach_command {
            attach_hooks.push(
                hook.try_substitute(&|s| config::vars::resolve_container_env(s, &container_env))?,
            );
        }
        let scripts = config
            .scripts
            .iter()
            .map(|(name, cmd)| Ok((name.clone(), substitute(cmd)?)))
            .collect::<anyhow::Result<_>>()?;
        let feature_scripts = feature_runtime
            .feature_scripts
            .iter()
            .map(|(name, scripts)| {
                let scripts = scripts
                    .iter()
                    .map(|(name, cmd)| Ok((name.clone(), substitute(cmd)?)))
                    .collect::<anyhow::Result<_>>()?;
                Ok((name.clone(), scripts))
            })
            .collect::<anyhow::Result<_>>()?;
        let fingerprint = match crate::runtime_snapshot::fingerprint(
            config_path,
            &workspace.root,
            &cache_dir.host_path,
            metadata.as_deref().unwrap_or_default(),
        ) {
            Ok(hash) => Some(hash),
            Err(_) => {
                eprintln!("warning: launch configuration fingerprint unavailable; later comparisons will be incomplete");
                None
            }
        };
        let snapshot = crate::runtime_snapshot::RuntimeSnapshot {
            protocol: 1,
            identity: container_id.as_str().to_owned(),
            token: launch_token,
            image: image_tag.clone(),
            fingerprint,
            user: config.container_user.clone(),
            workdir: config.workspace_folder.clone(),
            local_workspace: local_workspace.clone(),
            local_cache: local_cache.clone(),
            container_env: container_env.clone(),
            attach_hooks,
            scripts,
            feature_scripts,
            ports,
            memory: opts.limits_memory.clone(),
            cpus: opts.limits_cpus.clone(),
        };
        rt_dir.write_snapshot(&snapshot)?;

        if opts.debug {
            eprintln!(
                "dcc: creating container {} from {} with {} port mappings",
                container.as_str(),
                image_tag,
                snapshot.ports.len()
            );
        }

        Ok(Self {
            _assets: rt_dir,
            container,
            run_args: args,
            snapshot,
        })
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum RuntimeHookPhase {
    Startup,
    Attach,
}

impl RuntimeHookPhase {
    fn hook_name(self) -> &'static str {
        match self {
            Self::Startup => "postStartCommand",
            Self::Attach => "postAttachCommand",
        }
    }

    fn get(self, hooks: &lifecycle::LifecycleHooks) -> &Option<lifecycle::LifecycleCommand> {
        match self {
            Self::Startup => &hooks.post_start_command,
            Self::Attach => &hooks.post_attach_command,
        }
    }
}

/// Builds the warning messages for lifecycle hooks skipped under `--skip-lifecycle`,
/// for a single runtime phase.
fn skipped_hook_warnings(
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    phase: RuntimeHookPhase,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let name = phase.hook_name();
    for (feature_id, hooks) in &feature_runtime.feature_hooks {
        if phase.get(hooks).is_some() {
            warnings.push(format!(
                "skipping {name} from feature `{feature_id}` (--skip-lifecycle)"
            ));
        }
    }
    if phase.get(&config.lifecycle).is_some() {
        warnings.push(format!("skipping {name} (--skip-lifecycle)"));
    }
    warnings
}

fn resolve_runtime_state(
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    container_env: &std::collections::HashMap<String, String>,
) -> anyhow::Result<Vec<config::StateEntry>> {
    let state: Vec<config::StateEntry> = feature_runtime
        .state
        .iter()
        .cloned()
        .chain(config.state.iter().cloned())
        .collect();
    config::resolve::resolve_state_entries_container_env(&state, container_env)
}

/// Best-effort runtime seed guard (start/run/exec/attach). Compares the host-side
/// ledger's `build_id` against the image's `dcc.seed` label and warns on mismatch;
/// hydrates only entries with no ledger record at all (the wiped-`.dcc` recovery
/// case). Content re-digesting is off the hot path by design. Failures are
/// warnings, not hard errors, so a stale ledger never blocks the runtime.
async fn runtime_seed_guard(
    cache_dir: &CacheDir,
    image: &str,
    state: &[config::StateEntry],
    container_user: &str,
) {
    let ledger_path = crate::seed::SeedLedger::path(cache_dir);
    let ledger = match crate::seed::SeedLedger::read(&ledger_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "warning: could not read seed ledger (`{}`): {e:#}",
                ledger_path.display()
            );
            return;
        }
    };

    let image_manifest = match crate::seed::read_manifest_from_image(image, image).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("warning: could not read dcc.seed label from image `{image}`: {e:#}");
            return;
        }
    };

    // Warn on build_id mismatch (image rebuilt but ledger is stale).
    if !image_manifest.is_empty() && image_manifest.build_id != image {
        // build_id in the label should equal the image tag; mismatch means the
        // image was rebuilt under the same tag with different state.
        eprintln!(
            "warning: seed ledger build_id `{}` does not match image `{}` dcc.seed build_id `{}`; \
             declared state may be stale. Run `dcc build` to re-seed.",
            ledger
                .entries
                .first()
                .map(|e| e.build_id.as_str())
                .unwrap_or("(none)"),
            image,
            image_manifest.build_id
        );
    }

    // Hydrate only entries with no ledger record at all (wiped-.dcc recovery).
    let unseeded: Vec<config::StateEntry> = state
        .iter()
        .filter(|entry| ledger.get(&entry.path).is_none())
        .cloned()
        .collect();
    if unseeded.is_empty() {
        return;
    }

    eprintln!(
        "dcc: {} declared state path(s) have no seed record; hydrating from image `{image}`",
        unseeded.len()
    );
    let manifest = crate::seed::manifest_from_state(&unseeded, image);
    let state_root = crate::seed::state_root(cache_dir);
    if std::fs::create_dir_all(&state_root).is_err() {
        eprintln!(
            "warning: could not create state root `{}` for hydration",
            state_root.display()
        );
        return;
    }
    let state_root_str = state_root.to_string_lossy().into_owned();
    let entries: Vec<crate::seed::SeedManifestEntry> = manifest.entries.clone();
    let owner = (container_user != "root").then_some(container_user);
    let args = crate::seed::hydration_container_args(image, &state_root_str, &entries, owner);
    if let Err(e) = crate::docker::run_to_completion(&args).await {
        eprintln!("warning: state hydration from image `{image}` failed: {e:#}");
        return;
    }
    // Record ledger entries for the newly hydrated paths.
    let mut new_entries = ledger.entries.clone();
    for entry in &manifest.entries {
        let host_path = crate::seed::state_host_path(cache_dir, &entry.path);
        let digest = crate::seed::host_state_digest(&host_path).ok().flatten();
        new_entries.push(crate::seed::LedgerEntry {
            path: entry.path.clone(),
            kind: entry.kind,
            seed_digest: digest,
            build_id: manifest.build_id.clone(),
        });
    }
    if let Err(e) = (crate::seed::SeedLedger {
        entries: new_entries,
    })
    .write(&ledger_path)
    {
        eprintln!("warning: failed to update seed ledger: {e:#}");
    }
}

fn ensure_unsafe_runtime_allowed(
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    allow_unsafe_runtime: bool,
) -> anyhow::Result<()> {
    if allow_unsafe_runtime
        || (config.unsafe_runtime.is_empty() && feature_runtime.unsafe_runtime.is_empty())
    {
        return Ok(());
    }
    if !config.unsafe_runtime.is_empty() {
        anyhow::bail!(
            "devcontainer config contains unsafe runtime setting(s) {}; rerun with `--allow-unsafe-runtime` to allow them",
            config.unsafe_runtime.property_names().join(", ")
        );
    }
    anyhow::bail!(
        "image metadata contains unsafe Feature runtime setting(s) {}; rerun with `--allow-unsafe-runtime` to allow them",
        unsafe_runtime_property_names(&feature_runtime.unsafe_runtime).join(", ")
    );
}

fn append_unsafe_runtime_args(
    args: &mut Vec<String>,
    config_unsafe_runtime: &config::UnsafeRuntimeConfig,
    unsafe_runtime: &FeatureUnsafeRuntime,
    allow_unsafe_runtime: bool,
) {
    if !allow_unsafe_runtime {
        return;
    }
    if config_unsafe_runtime.privileged {
        args.push("--privileged".to_string());
    }
    for cap in &config_unsafe_runtime.cap_add {
        args.push("--cap-add".to_string());
        args.push(cap.clone());
    }
    for opt in &config_unsafe_runtime.security_opt {
        args.push("--security-opt".to_string());
        args.push(opt.clone());
    }
    if unsafe_runtime.privileged {
        args.push("--privileged".to_string());
    }
    for cap in &unsafe_runtime.cap_add {
        args.push("--cap-add".to_string());
        args.push(cap.clone());
    }
    for opt in &unsafe_runtime.security_opt {
        args.push("--security-opt".to_string());
        args.push(opt.clone());
    }
}

fn unsafe_runtime_property_names(unsafe_runtime: &FeatureUnsafeRuntime) -> Vec<&'static str> {
    let mut names = Vec::new();
    if unsafe_runtime.privileged {
        names.push("privileged");
    }
    if !unsafe_runtime.cap_add.is_empty() {
        names.push("capAdd");
    }
    if !unsafe_runtime.security_opt.is_empty() {
        names.push("securityOpt");
    }
    names
}

fn sanitize_run_args(args: &[String], allow_unsafe_runtime: bool) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg.is_empty() || !arg.starts_with('-') {
            anyhow::bail!(
                "unsupported runArgs entry `{arg}`; runArgs must contain docker run flags only"
            );
        }

        if arg == "--privileged" {
            require_unsafe_run_arg(arg, allow_unsafe_runtime)?;
            out.push(arg.clone());
            i += 1;
            continue;
        }

        if let Some((flag, value)) = split_equals_flag(arg) {
            handle_run_arg_value(flag, value, arg, allow_unsafe_runtime, &mut out)?;
            i += 1;
            continue;
        }

        match arg.as_str() {
            "--cap-add" | "--security-opt" | "--device" | "--pid" | "--ipc" | "--network"
            | "--mount" | "-v" | "--volume" | "--label" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("runArgs flag `{arg}` requires a value"))?;
                handle_run_arg_value(arg, value, arg, allow_unsafe_runtime, &mut out)?;
                i += 2;
            }
            "--add-host" | "--dns" | "--dns-search" | "--dns-option" | "--hostname" | "--tmpfs"
            | "--shm-size" | "--ulimit" | "--platform" | "--cap-drop" | "--stop-signal" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("runArgs flag `{arg}` requires a value"))?;
                out.push(arg.clone());
                out.push(value.clone());
                i += 2;
            }
            "-e" | "--env" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("runArgs flag `{arg}` requires a value"))?;
                ensure_explicit_env_value(arg, value)?;
                out.push(arg.clone());
                out.push(value.clone());
                i += 2;
            }
            _ => {
                anyhow::bail!(
                    "unsupported runArgs flag `{arg}`; dcc only passes a conservative safe subset by default"
                );
            }
        }
    }
    Ok(out)
}

fn split_equals_flag(arg: &str) -> Option<(&str, &str)> {
    let (flag, value) = arg.split_once('=')?;
    if flag.starts_with("--") {
        Some((flag, value))
    } else {
        None
    }
}

fn handle_run_arg_value(
    flag: &str,
    value: &str,
    original: &str,
    allow_unsafe_runtime: bool,
    out: &mut Vec<String>,
) -> anyhow::Result<()> {
    match flag {
        "--cap-add" | "--security-opt" | "--device" => {
            require_unsafe_run_arg(flag, allow_unsafe_runtime)?;
        }
        "--pid" | "--ipc" => {
            if value == "host" {
                require_unsafe_run_arg(original, allow_unsafe_runtime)?;
            } else {
                anyhow::bail!(
                    "unsupported runArgs flag `{original}`; only `host` mode is recognized and requires `--allow-unsafe-runtime`"
                );
            }
        }
        "--network" => {
            if value == "host" {
                require_unsafe_run_arg(original, allow_unsafe_runtime)?;
            } else if !matches!(value, "bridge" | "none" | "default") {
                anyhow::bail!(
                    "unsupported runArgs network mode `{value}`; supported safe modes are bridge, none, and default"
                );
            }
        }
        "--mount" => {
            if mount_value_is_sensitive(value) {
                require_unsafe_run_arg(original, allow_unsafe_runtime)?;
            }
        }
        "-v" | "--volume" => {
            if volume_value_is_sensitive(value) {
                require_unsafe_run_arg(original, allow_unsafe_runtime)?;
            }
        }
        "-e" | "--env" => ensure_explicit_env_value(flag, value)?,
        "--label" => ensure_label_not_reserved(value)?,
        "--add-host" | "--dns" | "--dns-search" | "--dns-option" | "--hostname" | "--tmpfs"
        | "--shm-size" | "--ulimit" | "--platform" | "--cap-drop" | "--stop-signal" => {}
        _ => {
            anyhow::bail!(
                "unsupported runArgs flag `{flag}`; dcc only passes a conservative safe subset by default"
            );
        }
    }

    if original.contains('=') {
        out.push(original.to_string());
    } else {
        out.push(flag.to_string());
        out.push(value.to_string());
    }
    Ok(())
}

fn ensure_label_not_reserved(value: &str) -> anyhow::Result<()> {
    let key = value.split_once('=').map_or(value, |(key, _)| key);
    if key == docker::CONTAINER_ID_LABEL
        || key == docker::CONTAINER_ROLE_LABEL
        || key == "dcc.launch_token"
    {
        anyhow::bail!("runArgs label `{key}` is reserved for dcc container lifecycle metadata");
    }
    Ok(())
}

fn ensure_explicit_env_value(flag: &str, value: &str) -> anyhow::Result<()> {
    if value.contains('=') {
        return Ok(());
    }
    anyhow::bail!(
        "runArgs flag `{flag}` must use an explicit KEY=VALUE pair; host environment passthrough is not allowed"
    )
}

fn require_unsafe_run_arg(arg: &str, allow_unsafe_runtime: bool) -> anyhow::Result<()> {
    if allow_unsafe_runtime {
        return Ok(());
    }
    anyhow::bail!(
        "runArgs contains unsafe runtime flag `{arg}`; rerun with `--allow-unsafe-runtime` to allow it"
    )
}

fn ensure_mounts_safe(mounts: &[String], allow_unsafe_runtime: bool) -> anyhow::Result<()> {
    if allow_unsafe_runtime {
        return Ok(());
    }
    for mount in mounts {
        if mount_value_is_sensitive(mount) {
            anyhow::bail!(
                "mount `{mount}` exposes a sensitive host path; rerun with `--allow-unsafe-runtime` to allow it"
            );
        }
    }
    Ok(())
}

fn mount_value_is_sensitive(mount: &str) -> bool {
    parse_bind_src(mount)
        .as_deref()
        .is_some_and(is_sensitive_host_source)
        || parse_bind_dst(mount)
            .as_deref()
            .is_some_and(is_sensitive_mount_target)
}

fn volume_value_is_sensitive(value: &str) -> bool {
    volume_source(value).is_some_and(is_sensitive_host_source)
        || volume_target(value).is_some_and(is_sensitive_mount_target)
}

fn volume_source(value: &str) -> Option<&str> {
    if value.starts_with(':') {
        return None;
    }
    let (src, _rest) = value.split_once(':')?;
    if src.starts_with('/') || src.starts_with('~') {
        Some(src)
    } else {
        None
    }
}

fn volume_target(value: &str) -> Option<&str> {
    let mut parts = value.splitn(3, ':');
    let _src = parts.next()?;
    parts.next()
}

fn is_sensitive_host_source(src: &str) -> bool {
    let trimmed = src.trim();
    if has_parent_dir_component(trimmed) {
        return true;
    }
    if matches!(trimmed, "/" | "/etc" | "/var/run" | "/var/run/") {
        return true;
    }
    if trimmed == "/var/run/docker.sock" || trimmed.ends_with("/docker.sock") {
        return true;
    }
    if trimmed.starts_with("/etc/") || trimmed.starts_with("/var/run/") {
        return true;
    }
    trimmed.contains("/.ssh/") || trimmed.ends_with("/.ssh") || is_ssh_agent_path(trimmed)
}

fn is_sensitive_mount_target(target: &str) -> bool {
    is_ssh_agent_path(target.trim())
}

fn has_parent_dir_component(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn is_ssh_agent_path(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    path.contains("ssh_auth_sock")
        || path.contains("ssh-agent")
        || path.contains("/ssh-")
        || path.contains("/agent.")
        || path.ends_with("/ssh")
        || path.ends_with("/agent")
}

/// Returns true when any runtime-applied field references `${containerEnv:…}`. Used to
/// gate the HOME/USER probe so configs that don't use containerEnv pay no extra cost.
fn references_container_env(
    override_args: &[String],
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
) -> bool {
    const NEEDLE: &str = "${containerEnv:";
    let has = |s: &str| s.contains(NEEDLE);

    if override_args.iter().any(|s| has(s)) {
        return true;
    }
    if config.workspace_folder.contains(NEEDLE) || config.run_args.iter().any(|s| has(s)) {
        return true;
    }
    if config.mounts.iter().any(|s| has(s)) || feature_runtime.mounts.iter().any(|s| has(s)) {
        return true;
    }
    if config.state.iter().any(|entry| has(&entry.path)) {
        return true;
    }
    if feature_runtime.state.iter().any(|entry| has(&entry.path)) {
        return true;
    }
    if config.remote_env.values().any(|s| has(s))
        || feature_runtime.remote_env.values().any(|s| has(s))
    {
        return true;
    }
    // Runtime startup/attach hooks from both devcontainer.json and features.
    // Build-prep hooks and unsupported host hooks are intentionally excluded from
    // ordinary runtime commands.
    let mut cmds: Vec<&lifecycle::LifecycleCommand> = Vec::new();
    for phase in [RuntimeHookPhase::Startup, RuntimeHookPhase::Attach] {
        cmds.extend(phase.get(&config.lifecycle));
        for (_id, hooks) in &feature_runtime.feature_hooks {
            cmds.extend(phase.get(hooks));
        }
    }
    cmds.into_iter()
        .any(|c| has(&describe_lifecycle_command(c)))
}

/// Renders a lifecycle command for `--debug` output: a shell string as-is, an
/// argv joined by spaces, and an object (parallel) form as `name: cmd` entries.
fn describe_lifecycle_command(cmd: &lifecycle::LifecycleCommand) -> String {
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

/// Prints a user-facing warning for a value that still contains a `${...}`
/// reference after substitution. dcc writes user-facing diagnostics straight to
/// stderr (like the top-level error in `main`) rather than through `tracing`,
/// which is silent unless `RUST_LOG` is set.
fn warn_unresolved_variables(kind: &str, value: &str) {
    let unresolved = config::vars::unresolved_variables(value);
    if unresolved.is_empty() {
        return;
    }
    eprintln!(
        "warning: {kind} contains {} unresolved variable reference(s); \
         dcc substitutes ${{localWorkspaceFolder}}, ${{localCacheFolder}}, \
         ${{containerWorkspaceFolder}}, ${{containerCacheFolder}}, ${{localEnv:VAR}}, \
         and ${{containerEnv:VAR}}",
        unresolved.len()
    );
}

// Restricted to the cache directory (dcc-managed space) to avoid silently creating
// arbitrary host paths that would mask misconfigurations like typos pointing at ~/.ssh.
fn ensure_cache_mount_sources(mounts: &[String], cache_dir: &CacheDir) -> anyhow::Result<()> {
    for mount in mounts {
        let Some(src) = parse_bind_src(mount) else {
            continue;
        };
        if has_parent_dir_component(&src) {
            anyhow::bail!(
                "mount source `{src}` contains parent directory segments; dcc will not create cache mount sources through non-normalized paths"
            );
        }
        if Path::new(&src).starts_with(&cache_dir.host_path) {
            std::fs::create_dir_all(&src)
                .with_context(|| format!("failed to create mount source directory `{src}`"))?;
        }
    }
    Ok(())
}

/// Extracts the source path from a `type=bind` Docker mount string, or returns `None`.
///
/// Accepts both `src=` and `source=` key spellings. Returns `None` for volume/tmpfs mounts
/// or bind mounts with no explicit source.
fn parse_bind_src(mount: &str) -> Option<String> {
    parse_bind_field(mount, &["src=", "source="])
}

fn parse_bind_dst(mount: &str) -> Option<String> {
    parse_bind_field(mount, &["dst=", "destination=", "target="])
}

fn parse_bind_field(mount: &str, keys: &[&str]) -> Option<String> {
    let mut is_bind = false;
    let mut value: Option<&str> = None;
    for part in mount.split(',') {
        let part = part.trim();
        if part == "type=bind" {
            is_bind = true;
        } else if let Some(v) = keys.iter().find_map(|key| part.strip_prefix(key)) {
            value = Some(v);
        }
    }
    if is_bind {
        value.map(str::to_owned)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{profile::ProfileName, workspace::Workspace};

    // --- parse_bind_src ---

    #[test]
    fn parse_bind_src_standard() {
        assert_eq!(
            parse_bind_src("type=bind,src=/host/path,dst=/container/path"),
            Some("/host/path".to_owned())
        );
    }

    #[test]
    fn parse_bind_src_source_synonym() {
        assert_eq!(
            parse_bind_src("type=bind,source=/host/path,target=/container/path"),
            Some("/host/path".to_owned())
        );
    }

    #[test]
    fn parse_bind_src_src_before_type() {
        assert_eq!(
            parse_bind_src("src=/host,type=bind,dst=/container"),
            Some("/host".to_owned())
        );
    }

    #[test]
    fn parse_bind_src_with_readonly() {
        assert_eq!(
            parse_bind_src("type=bind,src=/host,dst=/container,readonly"),
            Some("/host".to_owned())
        );
    }

    #[test]
    fn parse_bind_src_volume_returns_none() {
        assert_eq!(
            parse_bind_src("type=volume,source=myvolume,target=/data"),
            None
        );
    }

    #[test]
    fn parse_bind_src_no_type_returns_none() {
        assert_eq!(parse_bind_src("src=/path,dst=/dst"), None);
    }

    #[test]
    fn parse_bind_src_tmpfs_returns_none() {
        assert_eq!(parse_bind_src("type=tmpfs,dst=/tmp"), None);
    }

    #[test]
    fn teardown_wait_only_applies_to_new_one_shot_containers() {
        assert!(should_wait_for_one_shot_teardown(true, false));
        assert!(!should_wait_for_one_shot_teardown(true, true));
        assert!(!should_wait_for_one_shot_teardown(false, false));
        assert!(!should_wait_for_one_shot_teardown(false, true));
    }

    // --- ensure_cache_mount_sources ---

    fn make_cache(root: &std::path::Path) -> CacheDir {
        CacheDir::new(
            &Workspace {
                root: root.to_path_buf(),
                identity: root.to_string_lossy().into_owned(),
            },
            &ProfileName::new("dev"),
        )
    }

    #[test]
    fn creates_missing_subdir_under_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = make_cache(tmp.path());
        let src = cache.host_path.join("node_modules");
        let mount = format!(
            "type=bind,src={},dst=/workspace/node_modules",
            src.display()
        );
        ensure_cache_mount_sources(&[mount], &cache).unwrap();
        assert!(src.is_dir());
    }

    #[test]
    fn does_not_create_path_outside_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = make_cache(tmp.path());
        let outside = tmp.path().join("outside");
        let mount = format!("type=bind,src={},dst=/container", outside.display());
        ensure_cache_mount_sources(&[mount], &cache).unwrap();
        assert!(!outside.exists());
    }

    #[test]
    fn idempotent_when_subdir_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = make_cache(tmp.path());
        let src = cache.host_path.join("cargo");
        std::fs::create_dir_all(&src).unwrap();
        let mount = format!("type=bind,src={},dst=/cache/cargo", src.display());
        // Should not error on second call
        ensure_cache_mount_sources(std::slice::from_ref(&mount), &cache).unwrap();
        ensure_cache_mount_sources(&[mount], &cache).unwrap();
        assert!(src.is_dir());
    }

    #[test]
    fn creates_nested_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = make_cache(tmp.path());
        let src = cache.host_path.join("a").join("b").join("c");
        let mount = format!("type=bind,src={},dst=/c", src.display());
        ensure_cache_mount_sources(&[mount], &cache).unwrap();
        assert!(src.is_dir());
    }

    #[test]
    fn skips_non_bind_mounts() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = make_cache(tmp.path());
        let src = cache.host_path.join("vol");
        let volume_mount = format!("type=volume,source={},target=/data", src.display());
        ensure_cache_mount_sources(&[volume_mount], &cache).unwrap();
        assert!(!src.exists());
    }

    #[test]
    fn path_starts_with_uses_components_not_string_prefix() {
        // A directory whose name is a prefix of the cache dir name should not match.
        // e.g. cache = /tmp/foo/.dcc/dev, outside = /tmp/foo/.dcc-extra/bar
        let tmp = tempfile::tempdir().unwrap();
        let cache = make_cache(tmp.path());
        // Construct a path that shares a string prefix with cache but is not under it
        let sibling_name = format!(
            "{}-extra",
            cache.host_path.file_name().unwrap().to_str().unwrap()
        );
        let sibling = cache.host_path.parent().unwrap().join(sibling_name);
        let outside = sibling.join("bar");
        let mount = format!("type=bind,src={},dst=/bar", outside.display());
        ensure_cache_mount_sources(&[mount], &cache).unwrap();
        assert!(!outside.exists());
    }

    // --- skipped_hook_warnings ---

    use crate::lifecycle::{LifecycleCommand, LifecycleHooks};
    use indexmap::IndexMap;
    use std::collections::HashMap;

    fn empty_config() -> config::DevcontainerConfig {
        config::DevcontainerConfig {
            name: None,
            image: Some("img".into()),
            build: None,
            features: IndexMap::new(),
            registry_cas: Default::default(),
            container_env: HashMap::new(),
            remote_env: HashMap::new(),
            container_user: "dev".into(),
            mounts: Vec::new(),
            run_args: Vec::new(),
            unsafe_runtime: config::UnsafeRuntimeConfig::default(),
            forward_ports: Vec::new(),
            relay_port_range: [20000, 20999],
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
    fn skipped_hook_warnings_empty_when_no_hooks() {
        let config = empty_config();
        let runtime = FeatureRuntimeConfig::default();
        assert!(skipped_hook_warnings(&config, &runtime, RuntimeHookPhase::Startup).is_empty());
        assert!(skipped_hook_warnings(&config, &runtime, RuntimeHookPhase::Attach).is_empty());
    }

    #[test]
    fn skipped_hook_warnings_lists_only_selected_runtime_phase() {
        let mut config = empty_config();
        config.lifecycle.post_start_command = shell("echo start");
        config.lifecycle.post_attach_command = shell("echo attach");
        config.lifecycle.on_create_command = shell("echo create");
        let runtime = FeatureRuntimeConfig::default();
        assert_eq!(
            skipped_hook_warnings(&config, &runtime, RuntimeHookPhase::Startup),
            vec!["skipping postStartCommand (--skip-lifecycle)".to_string()]
        );
        assert_eq!(
            skipped_hook_warnings(&config, &runtime, RuntimeHookPhase::Attach),
            vec!["skipping postAttachCommand (--skip-lifecycle)".to_string()]
        );
    }

    #[test]
    fn skipped_hook_warnings_feature_hook_named_and_ordered_before_devcontainer() {
        let mut config = empty_config();
        config.lifecycle.post_attach_command = shell("echo dc");
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.feature_hooks.push((
            "node".to_string(),
            LifecycleHooks {
                post_attach_command: shell("echo feat"),
                ..Default::default()
            },
        ));
        assert_eq!(
            skipped_hook_warnings(&config, &runtime, RuntimeHookPhase::Attach),
            vec![
                "skipping postAttachCommand from feature `node` (--skip-lifecycle)".to_string(),
                "skipping postAttachCommand (--skip-lifecycle)".to_string(),
            ]
        );
    }

    // --- describe_lifecycle_command ---

    #[test]
    fn describe_lifecycle_command_renders_each_form() {
        use crate::lifecycle::LifecycleCommandSingle;
        assert_eq!(
            describe_lifecycle_command(&LifecycleCommand::Shell("echo hi".into())),
            "echo hi"
        );
        assert_eq!(
            describe_lifecycle_command(&LifecycleCommand::Exec(vec!["echo".into(), "hi".into()])),
            "echo hi"
        );
        let mut map = IndexMap::new();
        map.insert("a".to_string(), LifecycleCommandSingle::Shell("x".into()));
        map.insert(
            "b".to_string(),
            LifecycleCommandSingle::Exec(vec!["y".into(), "z".into()]),
        );
        assert_eq!(
            describe_lifecycle_command(&LifecycleCommand::Parallel(map)),
            "a: x | b: y z"
        );
    }

    // --- references_container_env ---

    #[test]
    fn references_container_env_false_when_absent() {
        let config = empty_config();
        assert!(!references_container_env(
            &["ls".to_string()],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    #[test]
    fn references_container_env_true_in_mount() {
        let mut config = empty_config();
        config
            .mounts
            .push("type=bind,src=${containerEnv:HOME}/.cache,dst=/c".to_string());
        assert!(references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    #[test]
    fn references_container_env_true_in_state() {
        let mut config = empty_config();
        config.state.push(config::StateEntry {
            path: "${containerEnv:HOME}/.cache".to_string(),
            kind: config::StateKind::Directory,
        });
        assert!(references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    #[test]
    fn references_container_env_true_in_feature_state() {
        let config = empty_config();
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.state.push(config::StateEntry {
            path: "${containerEnv:HOME}/.cache".to_string(),
            kind: config::StateKind::Directory,
        });
        assert!(references_container_env(&[], &config, &runtime));
    }

    #[test]
    fn references_container_env_true_in_override_args() {
        let config = empty_config();
        assert!(references_container_env(
            &["echo".to_string(), "${containerEnv:USER}".to_string()],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    #[test]
    fn references_container_env_true_in_hook() {
        let mut config = empty_config();
        config.lifecycle.post_start_command = shell("echo ${containerEnv:HOME}");
        assert!(references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    #[test]
    fn references_container_env_false_for_build_prep_hook_only() {
        let mut config = empty_config();
        config.lifecycle.post_create_command = shell("echo ${containerEnv:HOME}");
        assert!(!references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    #[test]
    fn references_container_env_false_for_initialize_command() {
        let mut config = empty_config();
        config.initialize_command = Some(LifecycleCommand::Shell(
            "echo ${containerEnv:HOME}".to_string(),
        ));
        assert!(!references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    #[test]
    fn references_container_env_true_in_run_args_and_workspace_folder() {
        let mut config = empty_config();
        config
            .run_args
            .push("--label=home=${containerEnv:HOME}".to_string());
        assert!(references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));

        let mut config = empty_config();
        config.workspace_folder = "${containerEnv:HOME}/project".to_string();
        assert!(references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }

    // --- runtime state and unsafe Feature settings ---

    #[test]
    fn resolve_runtime_state_merges_feature_state_before_project_state() {
        let mut config = empty_config();
        config.state.push(config::StateEntry {
            path: "/workspace/target".to_string(),
            kind: config::StateKind::Directory,
        });
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.state.push(config::StateEntry {
            path: "/home/dev/.cargo".to_string(),
            kind: config::StateKind::Directory,
        });
        let env = HashMap::new();
        let state = resolve_runtime_state(&config, &runtime, &env).unwrap();
        assert_eq!(
            state,
            vec![
                config::StateEntry {
                    path: "/home/dev/.cargo".to_string(),
                    kind: config::StateKind::Directory,
                },
                config::StateEntry {
                    path: "/workspace/target".to_string(),
                    kind: config::StateKind::Directory,
                },
            ]
        );
    }

    #[test]
    fn resolve_runtime_state_rejects_feature_project_overlap() {
        let mut config = empty_config();
        config.state.push(config::StateEntry {
            path: "/home/dev/.cache/tool".to_string(),
            kind: config::StateKind::Directory,
        });
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.state.push(config::StateEntry {
            path: "/home/dev/.cache".to_string(),
            kind: config::StateKind::Directory,
        });
        let env = HashMap::new();
        let err = resolve_runtime_state(&config, &runtime, &env).unwrap_err();
        assert!(err.to_string().contains("overlap"), "got: {err:#}");
    }

    #[test]
    fn unsafe_runtime_rejected_without_flag() {
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.unsafe_runtime.privileged = true;
        let config = empty_config();
        let err = ensure_unsafe_runtime_allowed(&config, &runtime, false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("--allow-unsafe-runtime"), "got: {msg}");
        assert!(msg.contains("privileged"), "got: {msg}");
    }

    #[test]
    fn devcontainer_unsafe_runtime_rejected_without_flag() {
        let mut config = empty_config();
        config.unsafe_runtime.cap_add.push("SYS_PTRACE".to_string());
        let runtime = FeatureRuntimeConfig::default();
        let err = ensure_unsafe_runtime_allowed(&config, &runtime, false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("devcontainer config"), "got: {msg}");
        assert!(msg.contains("capAdd"), "got: {msg}");
        assert!(msg.contains("--allow-unsafe-runtime"), "got: {msg}");
    }

    #[test]
    fn unsafe_runtime_appended_only_with_flag() {
        let mut config_unsafe = config::UnsafeRuntimeConfig::default();
        config_unsafe.security_opt.push("label=disable".to_string());
        let unsafe_runtime = FeatureUnsafeRuntime {
            privileged: true,
            cap_add: vec!["SYS_PTRACE".to_string()],
            security_opt: vec!["seccomp=unconfined".to_string()],
        };

        let mut args = Vec::new();
        append_unsafe_runtime_args(&mut args, &config_unsafe, &unsafe_runtime, false);
        assert!(args.is_empty());

        append_unsafe_runtime_args(&mut args, &config_unsafe, &unsafe_runtime, true);
        assert_eq!(
            args,
            vec![
                "--security-opt",
                "label=disable",
                "--privileged",
                "--cap-add",
                "SYS_PTRACE",
                "--security-opt",
                "seccomp=unconfined",
            ]
        );
    }

    #[test]
    fn sanitize_run_args_allows_safe_subset() {
        let args = vec![
            "--add-host".to_string(),
            "host.docker.internal:host-gateway".to_string(),
            "--dns=1.1.1.1".to_string(),
            "--network".to_string(),
            "none".to_string(),
            "-e".to_string(),
            "KEY=value".to_string(),
            "--label".to_string(),
            "service=api".to_string(),
            "--label=owner=dcc".to_string(),
            "--mount".to_string(),
            "type=bind,src=/home/me/project,dst=/project".to_string(),
        ];
        assert_eq!(sanitize_run_args(&args, false).unwrap(), args);
    }

    #[test]
    fn runtime_container_labels_include_identity_and_role() {
        let mut args = Vec::new();
        append_runtime_container_labels(&mut args, "dcc-id");
        assert_eq!(
            args,
            [
                "--label",
                "dcc.container_id=dcc-id",
                "--label",
                "dcc.container_role=runtime",
            ]
        );
    }

    #[test]
    fn sanitize_run_args_rejects_reserved_dcc_labels_in_all_supported_forms() {
        for args in [
            vec!["--label".to_string(), "dcc.container_id".to_string()],
            vec![
                "--label".to_string(),
                "dcc.container_role=build-prep".to_string(),
            ],
            vec!["--label=dcc.container_id=forged".to_string()],
            vec!["--label=dcc.container_role".to_string()],
        ] {
            for allow_unsafe_runtime in [false, true] {
                let err = sanitize_run_args(&args, allow_unsafe_runtime).unwrap_err();
                assert!(err.to_string().contains("reserved"), "got: {err:#}");
            }
        }
    }

    #[test]
    fn sanitize_run_args_rejects_host_env_passthrough() {
        let err =
            sanitize_run_args(&["--env".to_string(), "TOKEN".to_string()], false).unwrap_err();
        assert!(err.to_string().contains("KEY=VALUE"), "got: {err:#}");
    }

    #[test]
    fn sanitize_run_args_rejects_unknown_flag() {
        let err =
            sanitize_run_args(&["--entrypoint".to_string(), "sh".to_string()], false).unwrap_err();
        assert!(
            err.to_string().contains("unsupported runArgs"),
            "got: {err:#}"
        );
    }

    #[test]
    fn sanitize_run_args_gates_privileged_flags() {
        let args = vec!["--privileged".to_string()];
        let err = sanitize_run_args(&args, false).unwrap_err();
        assert!(err.to_string().contains("--allow-unsafe-runtime"));
        assert_eq!(sanitize_run_args(&args, true).unwrap(), args);
    }

    #[test]
    fn sanitize_run_args_gates_host_runtime_modes_and_devices() {
        for args in [
            vec!["--pid=host".to_string()],
            vec!["--ipc".to_string(), "host".to_string()],
            vec!["--network=host".to_string()],
            vec!["--device".to_string(), "/dev/kvm".to_string()],
            vec!["--cap-add".to_string(), "SYS_ADMIN".to_string()],
            vec!["--security-opt=seccomp=unconfined".to_string()],
        ] {
            let err = sanitize_run_args(&args, false).unwrap_err();
            assert!(err.to_string().contains("--allow-unsafe-runtime"));
            assert_eq!(sanitize_run_args(&args, true).unwrap(), args);
        }
    }

    #[test]
    fn sanitize_run_args_gates_sensitive_mounts() {
        for args in [
            vec![
                "--mount".to_string(),
                "type=bind,src=/var/run/docker.sock,dst=/var/run/docker.sock".to_string(),
            ],
            vec![
                "--mount".to_string(),
                "type=bind,src=/home/me/.ssh,dst=/host-ssh".to_string(),
            ],
            vec![
                "--mount".to_string(),
                "type=bind,src=/tmp/../etc,dst=/host-etc".to_string(),
            ],
            vec![
                "--mount".to_string(),
                "type=bind,src=/private/tmp/com.apple.launchd.X/listeners,dst=/ssh-agent"
                    .to_string(),
            ],
            vec!["-v".to_string(), "/:/host".to_string()],
            vec!["-v".to_string(), "/tmp/../etc:/host-etc".to_string()],
            vec!["--volume=/tmp/ssh-test/agent.123:/ssh-agent".to_string()],
        ] {
            let err = sanitize_run_args(&args, false).unwrap_err();
            assert!(err.to_string().contains("--allow-unsafe-runtime"));
            assert_eq!(sanitize_run_args(&args, true).unwrap(), args);
        }
    }

    #[test]
    fn ensure_mounts_safe_gates_sensitive_sources() {
        let safe = vec!["type=bind,src=/home/me/project,dst=/project".to_string()];
        ensure_mounts_safe(&safe, false).unwrap();

        let sensitive = vec!["type=bind,src=/etc,dst=/host-etc".to_string()];
        let err = ensure_mounts_safe(&sensitive, false).unwrap_err();
        assert!(err.to_string().contains("--allow-unsafe-runtime"));
        ensure_mounts_safe(&sensitive, true).unwrap();
    }

    #[test]
    fn ensure_mounts_safe_gates_parent_dir_escape_and_ssh_agent_target() {
        for mount in [
            "type=bind,src=/tmp/../etc,dst=/host-etc",
            "type=bind,src=/private/tmp/com.apple.launchd.X/listeners,dst=/ssh-agent",
        ] {
            let err = ensure_mounts_safe(&[mount.to_string()], false).unwrap_err();
            assert!(err.to_string().contains("--allow-unsafe-runtime"));
            ensure_mounts_safe(&[mount.to_string()], true).unwrap();
        }
    }

    #[test]
    fn ensure_cache_mount_sources_rejects_parent_dir_escape_under_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = make_cache(tmp.path());
        let src = cache.host_path.join("..").join("outside");
        let mount = format!("type=bind,src={},dst=/outside", src.display());
        let err = ensure_cache_mount_sources(&[mount], &cache).unwrap_err();
        assert!(
            err.to_string().contains("parent directory segments"),
            "got: {err:#}"
        );
        assert!(!tmp.path().join(".dcc").join("outside").exists());
    }
}
