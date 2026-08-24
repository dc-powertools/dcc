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
    pub(crate) memory: Option<&'a str>,
    pub(crate) cpus: Option<&'a str>,
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
    let plan = RuntimePlan::prepare(workspace, profile, config_path, &[], opts).await?;
    let existing =
        running_container_name(plan.container_id.as_str(), plan.container.as_str()).await?;
    if let Some(running) = &existing {
        // Promote an already-running container to durable via the supervisor.
        let status = docker::exec(
            running,
            &plan.config.container_user,
            &plan.config.workspace_folder,
            &[
                format!("{}/dcc-ctl", supervisor::DCC_SHARE),
                "mode".to_string(),
                "durable".to_string(),
            ],
        )
        .await
        .with_context(|| format!("failed to promote container `{running}` to durable"))?;
        if !status.success() {
            anyhow::bail!(
                "failed to promote container `{running}` to durable: dcc-ctl exited with status {}",
                status.code().unwrap_or(-1)
            );
        }
        if opts.debug {
            eprintln!(
                "dcc debug: container `{running}` already running, promoted to durable for `{}`",
                plan.container_id.as_str()
            );
        }
        return Ok(());
    }
    start_container(&plan).await?;
    // The supervisor runs postStartCommand hooks internally at startup. Wait for
    // bootstrap completion so a hook failure surfaces here as a clear error
    // rather than a hang, and so the container is ready when this returns.
    wait_ready(&plan, plan.container.as_str()).await
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

async fn execute_foreground(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    override_args: &[String],
    kind: ForegroundKind,
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
    let plan = RuntimePlan::prepare(workspace, profile, config_path, override_args, opts).await?;

    let (container_name, started) = async {
        let existing =
            running_container_name(plan.container_id.as_str(), plan.container.as_str()).await?;
        let (container_name, started) = if let Some(running) = existing {
            if opts.keep {
                let status = docker::exec(
                    &running,
                    &plan.config.container_user,
                    &plan.config.workspace_folder,
                    &[
                        format!("{}/dcc-ctl", supervisor::DCC_SHARE),
                        "mode".to_string(),
                        "durable".to_string(),
                    ],
                )
                .await
                .with_context(|| {
                    format!("failed to promote container `{running}` to durable")
                })?;
                if !status.success() {
                    anyhow::bail!(
                        "failed to promote container `{running}` to durable: dcc-ctl exited with status {}",
                        status.code().unwrap_or(-1)
                    );
                }
            }
            if opts.debug {
                eprintln!(
                    "dcc debug: using existing container `{running}` for `{}`",
                    plan.container_id.as_str()
                );
            }
            (running, false)
        } else {
            start_container(&plan).await?;
            (plan.container.as_str().to_string(), true)
        };
        if started {
            // The supervisor runs postStartCommand hooks internally at startup.
            // Wait for bootstrap completion so a hook failure surfaces as a
            // clear error rather than a hang. On reuse, hooks already ran.
            wait_ready(&plan, &container_name).await?;
        }
        anyhow::Ok((container_name, started))
    }
    .await?;

    let relay_handles = forward::forward_ports(&container_name, &plan.config.forward_ports)
        .await
        .with_context(|| {
            format!(
                "failed to set up port forwarding for container `{}`",
                container_name
            )
        })?;

    let command_result = async {
        if kind == ForegroundKind::Attach {
            run_runtime_hooks(&plan, &container_name, RuntimeHookPhase::Attach).await?;
        }
        // Run the user command through the dcc-exec wrapper so the supervisor can
        // register/deregister it. The wrapper exits with the command's status.
        let mut wrapped = vec![format!("{}/dcc-exec", supervisor::DCC_SHARE)];
        wrapped.extend(plan.command_args.iter().cloned());
        docker::exec_foreground(
            &container_name,
            &plan.config.container_user,
            &plan.config.workspace_folder,
            &wrapped,
            plan.tty,
        )
        .await
        .with_context(|| format!("failed to run command in container `{}`", container_name))
    }
    .await;

    relay_handles.shutdown().await;

    let status = command_result?;

    // One-shot containers created by this invocation self-teardown: the supervisor
    // drains and exits when the active set is empty, and Docker's --rm removes the
    // container. The host does not manage teardown state. We only wait until Docker's
    // label-based running-container query goes empty so immediate follow-up commands
    // and tests observe the same lifecycle boundary that reuse detection uses.
    if should_wait_for_one_shot_teardown(started, opts.keep) {
        wait_for_no_running_container(plan.container_id.as_str()).await;
    }
    Ok(status)
}

fn should_wait_for_one_shot_teardown(started: bool, keep: bool) -> bool {
    started && !keep
}

async fn start_container(plan: &RuntimePlan) -> anyhow::Result<()> {
    if plan.opts.debug {
        eprintln!(
            "dcc debug: starting container `{}`",
            plan.container.as_str()
        );
    }

    docker::start_detached(&plan.run_args)
        .await
        .with_context(|| format!("failed to start container `{}`", plan.container.as_str()))?;
    wait_for_running(plan.container.as_str())
        .await
        .with_context(|| format!("container `{}` failed to start", plan.container.as_str()))
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
    config: config::DevcontainerConfig,
    feature_runtime: FeatureRuntimeConfig,
    container_id: ContainerId,
    container: ContainerName,
    container_env: std::collections::HashMap<String, String>,
    local_workspace: String,
    local_cache: String,
    run_args: Vec<String>,
    command_args: Vec<String>,
    tty: bool,
    opts: OwnedExecOptions,
}

#[derive(Clone)]
struct OwnedExecOptions {
    limits_memory: Option<String>,
    limits_cpus: Option<String>,
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
            limits_memory: opts.limits.memory.map(str::to_string),
            limits_cpus: opts.limits.cpus.map(str::to_string),
            skip_lifecycle: opts.skip_lifecycle,
            debug: opts.debug,
            strict: opts.strict,
            profile_arg: opts.profile_arg.to_string(),
            allow_unsafe_runtime: opts.allow_unsafe_runtime,
            keep: opts.keep,
        }
    }
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
        let rt_dir = supervisor::RtDir::new(workspace, profile);

        let mut config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
            .with_context(|| format!("failed to load config `{}`", config_path.display()))?;

        let container_id = ContainerId::new(workspace, profile);
        let container = ContainerName::resolve(config.name.as_deref(), &container_id);
        let image_tag = container_id.as_image_tag();

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
        let feature_runtime = match docker::inspect_image_label(image_tag.as_str())
            .await
            .with_context(|| format!("failed to inspect image `{image_tag}`"))?
        {
            None => FeatureRuntimeConfig::default(),
            Some(ref json) => features::parse_runtime_from_label(json).with_context(|| {
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
        // leaves them unset, so the undefined-variable error below points at the cause.
        if references_container_env(override_args, &config, &feature_runtime) {
            match docker::probe_user_env(image_tag.as_str(), &config.container_user).await {
                Ok(probed) => container_env.extend(probed),
                Err(e) => eprintln!(
                    "warning: could not probe container HOME/USER ({e:#}); \
                 ${{containerEnv:HOME}}/${{containerEnv:USER}} may be unresolved"
                ),
            }
        }

        config.workspace_folder =
            config::vars::resolve_container_env(&config.workspace_folder, &container_env)
                .with_context(|| format!("in workspaceFolder `{}`", config.workspace_folder))?;
        config.run_args = config
            .run_args
            .iter()
            .map(|arg| {
                config::vars::resolve_container_env(arg, &container_env)
                    .with_context(|| format!("in runArgs entry `{arg}`"))
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
        let override_args: Vec<String> = override_args
            .iter()
            .map(|a| {
                let a = config::vars::apply_substitution(a, &local_workspace, &local_cache);
                config::vars::resolve_container_env(&a, &container_env)
                    .with_context(|| format!("in command argument `{a}`"))
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
            .map(|m| {
                config::vars::resolve_container_env(&m, &container_env)
                    .with_context(|| format!("in mount `{m}`"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        ensure_mounts_safe(&all_mounts, opts.allow_unsafe_runtime)?;
        ensure_cache_mount_sources(&all_mounts, &cache_dir)?;
        let safe_run_args = sanitize_run_args(&config.run_args, opts.allow_unsafe_runtime)?;

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
            .map(|(k, v)| {
                let resolved = config::vars::resolve_container_env(&v, &container_env)
                    .with_context(|| format!("in remoteEnv `{k}`"))?;
                anyhow::Ok((k, resolved))
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
        args.extend([
            "--label".into(),
            format!("dcc.container_id={}", container_id.as_str()),
        ]);
        args.extend([
            "--label".into(),
            format!("devcontainer.local_folder={}", workspace.root.display()),
        ]);
        args.extend([
            "--label".into(),
            format!("devcontainer.config_file={}", config_path.display()),
        ]);
        args.push("--rm".into());
        args.push("-dit".into());
        args.extend(["--workdir".into(), config.workspace_folder.clone()]);
        if let Some(memory) = &opts.limits_memory {
            args.extend(["--memory".into(), memory.clone()]);
        }
        if let Some(cpus) = &opts.limits_cpus {
            args.extend(["--cpus".into(), cpus.clone()]);
        }
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

        // Allocate a TTY for the foreground command only when our own stdin is a
        // terminal, so non-interactive use (pipes, CI) still works.
        let tty = std::io::stdin().is_terminal();

        // Print the fully-resolved launch picture before doing anything irreversible.
        if opts.debug {
            let mut dbg: Vec<String> = Vec::new();
            dbg.push(format!("── dcc debug {}", "─".repeat(40)));
            dbg.push(format!(
                "container : {}   image: {}",
                container.as_str(),
                image_tag.as_str()
            ));
            if container.as_str() != container_id.as_str() {
                dbg.push(format!("container id: {}", container_id.as_str()));
            }
            dbg.push(format!(
                "user: {}   memory: {}   cpus: {}   workdir: {}",
                config.container_user,
                opts.limits_memory.as_deref().unwrap_or("(unlimited)"),
                opts.limits_cpus.as_deref().unwrap_or("(unlimited)"),
                config.workspace_folder
            ));
            dbg.push(format!("command   : {}", override_args.join(" ")));

            dbg.push("remoteEnv (-e at runtime):".to_string());
            if remote_env.is_empty() {
                dbg.push("  (none)".to_string());
            } else {
                for (k, v) in &remote_env {
                    dbg.push(format!("  {k}={v}"));
                }
            }

            dbg.push("containerEnv (baked into image at build):".to_string());
            let mut cenv: Vec<(&String, &String)> = config.container_env.iter().collect();
            cenv.sort_by(|a, b| a.0.cmp(b.0));
            if cenv.is_empty() {
                dbg.push("  (none)".to_string());
            } else {
                for (k, v) in cenv {
                    dbg.push(format!("  {k}={v}"));
                }
            }

            dbg.push("mounts:".to_string());
            dbg.push(format!(
                "  bind   {local_workspace} -> {CONTAINER_WORKSPACE}"
            ));
            dbg.push(format!("  bind   {local_cache} -> {CONTAINER_CACHE}"));
            for m in &state_mount_args {
                dbg.push(format!("  {}", describe_mount(m)));
            }
            dbg.push(format!(
                "  bindro {} -> {}",
                rt_dir.host_path.display(),
                supervisor::RT_MOUNT
            ));
            dbg.push(format!("  tmpfs  -> {CONTAINER_WORKSPACE}/.dcc"));
            dbg.push(format!(
                "  tmpfs  -> {} (supervisor state)",
                supervisor::STATE_DIR
            ));
            for m in &all_mounts {
                dbg.push(format!("  {}", describe_mount(m)));
            }
            dbg.push(format!(
                "  entry  {}/dcc-supervisor (PID 1 supervisor, mode={mode})",
                supervisor::DCC_SHARE,
            ));

            dbg.push(format!(
                "forwardPorts: {}",
                if config.forward_ports.is_empty() {
                    "(none)".to_string()
                } else {
                    config
                        .forward_ports
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));

            dbg.push("lifecycle scripts:".to_string());
            dbg.extend(debug_lifecycle_lines(
                &config,
                &feature_runtime,
                opts.skip_lifecycle,
            ));

            dbg.push(format!("docker run {}", args.join(" ")));
            dbg.push(format!(
                "command runs via docker exec ({}): {}",
                if tty { "-it" } else { "-i" },
                override_args.join(" ")
            ));

            for line in dbg {
                eprintln!("{line}");
            }
        }

        Ok(Self {
            config,
            feature_runtime,
            container_id,
            container,
            container_env,
            local_workspace,
            local_cache,
            run_args: args,
            command_args: override_args,
            tty,
            opts,
        })
    }
}

async fn running_container_name(
    container_id: &str,
    fallback_container_name: &str,
) -> anyhow::Result<Option<String>> {
    if let Some(name) = docker::running_container_name_by_id(container_id).await? {
        return Ok(Some(name));
    }
    if docker::inspect_running(fallback_container_name).await? {
        return Ok(Some(fallback_container_name.to_string()));
    }
    if fallback_container_name != container_id && docker::inspect_running(container_id).await? {
        return Ok(Some(container_id.to_string()));
    }
    Ok(None)
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

/// Waits for the supervisor to finish startup (postStartCommand hooks) by running
/// `dcc-ctl wait-ready` inside the container. Maps a bootstrap failure (exit 252)
/// to a clear error so a failing hook surfaces here rather than as a hang.
async fn wait_ready(plan: &RuntimePlan, container: &str) -> anyhow::Result<()> {
    if plan.opts.skip_lifecycle {
        // Hooks were skipped; the supervisor writes bootstrap-status=0 immediately.
        // Still wait so the container is ready, but don't expect hook failures.
    }
    let status = docker::exec(
        container,
        &plan.config.container_user,
        &plan.config.workspace_folder,
        &[
            format!("{}/dcc-ctl", supervisor::DCC_SHARE),
            "wait-ready".to_string(),
        ],
    )
    .await
    .with_context(|| format!("failed to wait for container `{container}` readiness"))?;
    if !status.success() {
        if status.code() == Some(supervisor::EXIT_BOOTSTRAP_FAILED) {
            anyhow::bail!("container `{container}` startup failed: a postStartCommand hook failed");
        }
        anyhow::bail!(
            "container `{container}` startup failed: dcc-ctl wait-ready exited with status {}",
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
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

/// Runs the runtime lifecycle hooks for one phase. Feature-contributed hooks run
/// first, in feature installation order, followed by the devcontainer hook.
async fn run_runtime_hooks(
    plan: &RuntimePlan,
    container: &str,
    phase: RuntimeHookPhase,
) -> anyhow::Result<()> {
    if plan.opts.skip_lifecycle {
        for warning in skipped_hook_warnings(&plan.config, &plan.feature_runtime, phase) {
            eprintln!("warning: {warning}");
        }
        return Ok(());
    }

    // Feature hooks need host/localEnv substitution; `${containerEnv:…}` is then
    // resolved for both feature and devcontainer.json hooks. devcontainer.json
    // hooks were already host-substituted at config-load (containerEnv deferred).
    let substitute = |s: &str| -> anyhow::Result<String> {
        let s = config::vars::apply_substitution(s, &plan.local_workspace, &plan.local_cache);
        config::vars::resolve_container_env(&s, &plan.container_env)
    };
    let resolve_cenv = |s: &str| config::vars::resolve_container_env(s, &plan.container_env);
    let name = phase.hook_name();

    for (feature_id, hooks) in &plan.feature_runtime.feature_hooks {
        if let Some(cmd) = phase.get(hooks) {
            let cmd = cmd
                .try_substitute(&substitute)
                .with_context(|| format!("{name} from feature `{feature_id}`"))?;
            lifecycle::run_in_container(
                &cmd,
                container,
                &plan.config.container_user,
                &plan.config.workspace_folder,
            )
            .await
            .with_context(|| format!("{name} from feature `{feature_id}` failed"))?;
        }
    }

    if let Some(cmd) = phase.get(&plan.config.lifecycle) {
        let cmd = cmd
            .try_substitute(&resolve_cenv)
            .with_context(|| name.to_string())?;
        lifecycle::run_in_container(
            &cmd,
            container,
            &plan.config.container_user,
            &plan.config.workspace_folder,
        )
        .await
        .with_context(|| format!("{name} failed"))?;
    }

    Ok(())
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
            | "--mount" | "-v" | "--volume" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| anyhow::anyhow!("runArgs flag `{arg}` requires a value"))?;
                handle_run_arg_value(arg, value, arg, allow_unsafe_runtime, &mut out)?;
                i += 2;
            }
            "--add-host" | "--dns" | "--dns-search" | "--dns-option" | "--hostname" | "--label"
            | "--tmpfs" | "--shm-size" | "--ulimit" | "--platform" | "--cap-drop"
            | "--stop-signal" => {
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
                anyhow::bail!("unsupported runArgs flag `{original}`; only `host` mode is recognized and requires `--allow-unsafe-runtime`");
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
        "--add-host" | "--dns" | "--dns-search" | "--dns-option" | "--hostname" | "--label"
        | "--tmpfs" | "--shm-size" | "--ulimit" | "--platform" | "--cap-drop" | "--stop-signal" => {
        }
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

/// Renders a `docker --mount` string (`type=bind,src=…,dst=…,opts…`) into a
/// readable `type  src -> dst  [opts]` line for `--debug` output. Accepts the
/// `src`/`source` and `dst`/`destination`/`target` key spellings.
fn describe_mount(mount: &str) -> String {
    let mut typ = "";
    let mut src: Option<&str> = None;
    let mut dst: Option<&str> = None;
    let mut opts: Vec<&str> = Vec::new();
    for part in mount.split(',') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("type=") {
            typ = v;
        } else if let Some(v) = part
            .strip_prefix("src=")
            .or_else(|| part.strip_prefix("source="))
        {
            src = Some(v);
        } else if let Some(v) = part
            .strip_prefix("dst=")
            .or_else(|| part.strip_prefix("destination="))
            .or_else(|| part.strip_prefix("target="))
        {
            dst = Some(v);
        } else if !part.is_empty() {
            opts.push(part);
        }
    }
    let typ = if typ.is_empty() { "?" } else { typ };
    let mut line = match (src, dst) {
        (Some(s), Some(d)) => format!("{typ}  {s} -> {d}"),
        (None, Some(d)) => format!("{typ}  -> {d}"),
        (Some(s), None) => format!("{typ}  {s}"),
        (None, None) => typ.to_string(),
    };
    if !opts.is_empty() {
        line.push_str(&format!("  [{}]", opts.join(", ")));
    }
    line
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

/// Builds the `--debug` runtime lifecycle listing in execution order: startup
/// hooks, then attach hooks. Build-prep hooks and unsupported host hooks stay
/// out of ordinary runtime commands.
fn debug_lifecycle_lines(
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    skip_lifecycle: bool,
) -> Vec<String> {
    let suffix = if skip_lifecycle {
        "  (skipped: --skip-lifecycle)"
    } else {
        ""
    };
    let mut lines = Vec::new();
    for phase in [RuntimeHookPhase::Startup, RuntimeHookPhase::Attach] {
        let name = phase.hook_name();
        for (feature_id, hooks) in &feature_runtime.feature_hooks {
            if let Some(cmd) = phase.get(hooks) {
                lines.push(format!(
                    "  {name} (feature {feature_id}): {}{suffix}",
                    describe_lifecycle_command(cmd)
                ));
            }
        }
        if let Some(cmd) = phase.get(&config.lifecycle) {
            lines.push(format!(
                "  {name}: {}{suffix}",
                describe_lifecycle_command(cmd)
            ));
        }
    }
    if lines.is_empty() {
        lines.push("  (none)".to_string());
    }
    lines
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
        "warning: {kind} `{value}` references unresolved variable(s) {}; \
         dcc substitutes ${{localWorkspaceFolder}}, ${{localCacheFolder}}, \
         ${{containerWorkspaceFolder}}, ${{containerCacheFolder}}, ${{localEnv:VAR}}, \
         and ${{containerEnv:VAR}}",
        unresolved.join(", ")
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
            container_env: HashMap::new(),
            remote_env: HashMap::new(),
            container_user: "dev".into(),
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

    // --- describe_mount ---

    #[test]
    fn describe_mount_standard_bind() {
        assert_eq!(
            describe_mount("type=bind,src=/host,dst=/container"),
            "bind  /host -> /container"
        );
    }

    #[test]
    fn describe_mount_source_target_synonyms() {
        assert_eq!(
            describe_mount("type=bind,source=/h,target=/c"),
            "bind  /h -> /c"
        );
    }

    #[test]
    fn describe_mount_extra_options() {
        assert_eq!(
            describe_mount("type=bind,src=/h,dst=/c,readonly"),
            "bind  /h -> /c  [readonly]"
        );
    }

    #[test]
    fn describe_mount_tmpfs_has_no_source() {
        assert_eq!(describe_mount("type=tmpfs,dst=/tmp"), "tmpfs  -> /tmp");
    }

    #[test]
    fn describe_mount_volume() {
        assert_eq!(
            describe_mount("type=volume,source=vol,target=/data"),
            "volume  vol -> /data"
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

    // --- debug_lifecycle_lines ---

    #[test]
    fn debug_lifecycle_lines_empty() {
        assert_eq!(
            debug_lifecycle_lines(&empty_config(), &FeatureRuntimeConfig::default(), false),
            vec!["  (none)".to_string()]
        );
    }

    #[test]
    fn debug_lifecycle_lines_order_feature_then_devcontainer() {
        let mut config = empty_config();
        config.initialize_command = Some(LifecycleCommand::Shell("echo init".into()));
        config.lifecycle.post_start_command = shell("cargo fetch");
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.feature_hooks.push((
            "node".to_string(),
            LifecycleHooks {
                post_start_command: shell("npm ci"),
                ..Default::default()
            },
        ));
        assert_eq!(
            debug_lifecycle_lines(&config, &runtime, false),
            vec![
                "  postStartCommand (feature node): npm ci".to_string(),
                "  postStartCommand: cargo fetch".to_string(),
            ]
        );
    }

    #[test]
    fn debug_lifecycle_lines_annotates_skip() {
        let mut config = empty_config();
        config.lifecycle.post_attach_command = shell("x");
        assert_eq!(
            debug_lifecycle_lines(&config, &FeatureRuntimeConfig::default(), true),
            vec!["  postAttachCommand: x  (skipped: --skip-lifecycle)".to_string()]
        );
    }

    #[test]
    fn debug_lifecycle_lines_excludes_build_prep_hooks() {
        let mut config = empty_config();
        config.lifecycle.on_create_command = shell("create");
        config.lifecycle.update_content_command = shell("update");
        config.lifecycle.post_create_command = shell("post-create");
        assert_eq!(
            debug_lifecycle_lines(&config, &FeatureRuntimeConfig::default(), false),
            vec!["  (none)".to_string()]
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
            "--mount".to_string(),
            "type=bind,src=/home/me/project,dst=/project".to_string(),
        ];
        assert_eq!(sanitize_run_args(&args, false).unwrap(), args);
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
