use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config::{
        self,
        vars::{CONTAINER_CACHE, CONTAINER_WORKSPACE},
    },
    docker,
    features::{self, FeatureRuntimeConfig, FeatureUnsafeRuntime},
    forward, lifecycle,
    profile::{ContainerId, ContainerName, ProfileName},
    runtime::{ContainerMode, RuntimeState},
    version,
    workspace::Workspace,
};

/// CPU and memory limits forwarded to `docker run`.
#[derive(Clone, Copy)]
pub(crate) struct ResourceLimits<'a> {
    pub(crate) memory: &'a str,
    pub(crate) cpus: &'a str,
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
}

pub(crate) async fn exec(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    override_args: &[String],
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
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
    let plan = RuntimePlan::prepare(workspace, profile, config_path, &[], opts).await?;
    let state = RuntimeState::new(&plan.cache_dir);
    let _lock = state.acquire_lock()?;
    let existing =
        running_container_name(plan.container_id.as_str(), plan.container.as_str()).await?;
    state.set_mode(ContainerMode::Durable)?;
    if existing.is_some() {
        if opts.debug {
            eprintln!(
                "dcc debug: container `{}` already running for `{}`",
                existing.as_deref().unwrap_or(plan.container.as_str()),
                plan.container_id.as_str()
            );
        }
        return Ok(());
    }
    start_container(&plan, ContainerMode::Durable).await?;
    run_runtime_hooks(&plan, plan.container.as_str(), RuntimeHookPhase::Startup).await
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
    let state = RuntimeState::new(&plan.cache_dir);
    let active = state.create_active_command()?;
    let container_mode = if opts.keep {
        ContainerMode::Durable
    } else {
        ContainerMode::OneShot
    };

    let setup_result = async {
        let _lock = state.acquire_lock()?;
        let existing =
            running_container_name(plan.container_id.as_str(), plan.container.as_str()).await?;
        let (container_name, started) = if let Some(running) = existing {
            if opts.keep {
                state.set_mode(ContainerMode::Durable)?;
            }
            if opts.debug {
                eprintln!(
                    "dcc debug: using existing container `{running}` for `{}`",
                    plan.container_id.as_str()
                );
            }
            (running, false)
        } else {
            state.set_mode(container_mode)?;
            start_container(&plan, container_mode).await?;
            (plan.container.as_str().to_string(), true)
        };
        if started {
            run_runtime_hooks(&plan, &container_name, RuntimeHookPhase::Startup).await?;
        }
        anyhow::Ok(container_name)
    }
    .await;

    let container_name = match setup_result {
        Ok(container_name) => container_name,
        Err(e) => {
            let _ = finish_active_command(&state, &active, plan.container.as_str()).await;
            return Err(e);
        }
    };

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
        docker::exec_foreground(
            &container_name,
            &plan.config.container_user,
            CONTAINER_WORKSPACE,
            &plan.command_args,
            plan.tty,
        )
        .await
        .with_context(|| format!("failed to run command in container `{}`", container_name))
    }
    .await;

    for handle in relay_handles {
        handle.abort();
    }

    let stop_result = finish_active_command(&state, &active, &container_name).await;
    let status = command_result?;
    stop_result?;
    Ok(status)
}

async fn start_container(plan: &RuntimePlan, mode: ContainerMode) -> anyhow::Result<()> {
    // initializeCommand runs on the host before the container is created/started.
    if let Some(cmd) = &plan.config.initialize_command {
        if plan.opts.skip_lifecycle {
            eprintln!("warning: skipping initializeCommand (--skip-lifecycle)");
        } else {
            let cmd = cmd
                .try_substitute(&|s| config::vars::resolve_container_env(s, &plan.container_env))
                .context("initializeCommand")?;
            lifecycle::run_on_host(&cmd, &plan.workspace_root)
                .await
                .context("initializeCommand failed")?;
        }
    }

    if plan.opts.debug {
        eprintln!(
            "dcc debug: starting {} container `{}`",
            match mode {
                ContainerMode::OneShot => "one-shot",
                ContainerMode::Durable => "durable",
            },
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

async fn finish_active_command(
    state: &RuntimeState,
    active: &crate::runtime::ActiveCommand,
    container: &str,
) -> anyhow::Result<()> {
    let _lock = state.acquire_lock()?;
    state.complete_active_command(active)?;
    if state.mode() == ContainerMode::OneShot && state.active_count()? == 0 {
        docker::stop_container(container)
            .await
            .with_context(|| format!("failed to stop container `{container}`"))?;
        state.clear()?;
    }
    Ok(())
}

fn default_attach_command() -> Vec<String> {
    vec![
        "/bin/sh".to_string(),
        "-lc".to_string(),
        "if [ -n \"${SHELL:-}\" ] && [ \"${SHELL#/}\" != \"$SHELL\" ] && [ -x \"$SHELL\" ]; then exec \"$SHELL\"; elif [ -x /bin/bash ]; then exec /bin/bash; else exec /bin/sh; fi".to_string(),
    ]
}

struct RuntimePlan {
    workspace_root: PathBuf,
    cache_dir: CacheDir,
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
    limits_memory: String,
    limits_cpus: String,
    skip_lifecycle: bool,
    debug: bool,
    strict: bool,
    profile_arg: String,
    allow_unsafe_runtime: bool,
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

        let config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
            .with_context(|| format!("failed to load config `{}`", config_path.display()))?;

        let container_id = ContainerId::new(workspace, profile);
        let container = ContainerName::resolve(config.name.as_deref(), &container_id);
        let image_tag = container_id.as_image_tag();
        let current_uses_fast_path = crate::build::uses_fast_path(&config);

        // Ensure cache directory exists, then create any cache subdirectories
        // referenced as bind-mount sources (e.g. ${localCacheFolder}/node_modules).
        // Docker requires bind-mount source paths to exist on the host before startup.
        cache_dir.ensure_exists()?;

        version::warn_if_image_version_mismatch(
            image_tag.as_str(),
            Some(current_uses_fast_path),
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
        ensure_unsafe_runtime_allowed(&feature_runtime, opts.allow_unsafe_runtime)?;

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

        let state = resolve_runtime_state(&config, &feature_runtime, &container_env)
            .context("invalid customizations.dcc.state after resolving containerEnv")?;
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

        ensure_cache_mount_sources(&all_mounts, &cache_dir)?;

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
        args.extend(["--workdir".into(), CONTAINER_WORKSPACE.into()]);
        args.extend(["--memory".into(), opts.limits_memory.clone()]);
        args.extend(["--cpus".into(), opts.limits_cpus.clone()]);

        append_unsafe_runtime_args(
            &mut args,
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

        // mask .dcc directory inside container
        args.extend(["--tmpfs".into(), format!("{CONTAINER_WORKSPACE}/.dcc")]);

        // Keep-alive entrypoint: PID 1 must outlive the user command, which is run
        // separately in the foreground via `docker exec` below. Making the command PID 1
        // and attaching breaks for anything that exits quickly (e.g. `ls`) — the container
        // is gone before we can attach. `tail -f /dev/null` blocks forever and exists on
        // both glibc and BusyBox/Alpine images.
        args.extend(["--entrypoint".into(), "tail".into()]);

        // Image tag (must come after all flags)
        args.push(image_tag.as_str().to_owned());

        // Keep-alive command (arguments to the `tail` entrypoint)
        args.extend(["-f".into(), "/dev/null".into()]);

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
                "user: {}   memory: {}   cpus: {}   workdir: {CONTAINER_WORKSPACE}",
                config.container_user, opts.limits_memory, opts.limits_cpus
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
            dbg.push(format!("  tmpfs  -> {CONTAINER_WORKSPACE}/.dcc"));
            for m in &all_mounts {
                dbg.push(format!("  {}", describe_mount(m)));
            }

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
            workspace_root: workspace.root.clone(),
            cache_dir,
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
                CONTAINER_WORKSPACE,
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
            CONTAINER_WORKSPACE,
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

fn ensure_unsafe_runtime_allowed(
    feature_runtime: &FeatureRuntimeConfig,
    allow_unsafe_runtime: bool,
) -> anyhow::Result<()> {
    if allow_unsafe_runtime || feature_runtime.unsafe_runtime.is_empty() {
        return Ok(());
    }
    anyhow::bail!(
        "image metadata contains unsafe Feature runtime setting(s) {}; rerun with `--allow-unsafe-runtime` to allow them",
        unsafe_runtime_property_names(&feature_runtime.unsafe_runtime).join(", ")
    );
}

fn append_unsafe_runtime_args(
    args: &mut Vec<String>,
    unsafe_runtime: &FeatureUnsafeRuntime,
    allow_unsafe_runtime: bool,
) {
    if !allow_unsafe_runtime {
        return;
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
    // Lifecycle commands: host initializeCommand plus runtime startup/attach hooks
    // from both devcontainer.json and features. Build-prep hooks are intentionally
    // excluded from ordinary runtime commands.
    let mut cmds: Vec<&lifecycle::LifecycleCommand> = config.initialize_command.iter().collect();
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

/// Builds the `--debug` runtime lifecycle listing in execution order:
/// `initializeCommand` (host), startup hooks, then attach hooks. Build-prep hooks
/// stay out of ordinary runtime commands.
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
    if let Some(cmd) = &config.initialize_command {
        lines.push(format!(
            "  initializeCommand (host): {}{suffix}",
            describe_lifecycle_command(cmd)
        ));
    }
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
    let mut is_bind = false;
    let mut src: Option<&str> = None;
    for part in mount.split(',') {
        let part = part.trim();
        if part == "type=bind" {
            is_bind = true;
        } else if let Some(v) = part
            .strip_prefix("src=")
            .or_else(|| part.strip_prefix("source="))
        {
            src = Some(v);
        }
    }
    if is_bind {
        src.map(str::to_owned)
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
            forward_ports: Vec::new(),
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
    fn debug_lifecycle_lines_order_initialize_feature_then_devcontainer() {
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
                "  initializeCommand (host): echo init".to_string(),
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
        let err = ensure_unsafe_runtime_allowed(&runtime, false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("--allow-unsafe-runtime"), "got: {msg}");
        assert!(msg.contains("privileged"), "got: {msg}");
    }

    #[test]
    fn unsafe_runtime_appended_only_with_flag() {
        let unsafe_runtime = FeatureUnsafeRuntime {
            privileged: true,
            cap_add: vec!["SYS_PTRACE".to_string()],
            security_opt: vec!["seccomp=unconfined".to_string()],
        };

        let mut args = Vec::new();
        append_unsafe_runtime_args(&mut args, &unsafe_runtime, false);
        assert!(args.is_empty());

        append_unsafe_runtime_args(&mut args, &unsafe_runtime, true);
        assert_eq!(
            args,
            vec![
                "--privileged",
                "--cap-add",
                "SYS_PTRACE",
                "--security-opt",
                "seccomp=unconfined",
            ]
        );
    }
}
