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
    docker,
    features::{self, FeatureRuntimeConfig},
    forward, lifecycle,
    profile::{ContainerName, ProfileName},
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
}

pub(crate) async fn exec(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    override_args: &[String],
    opts: ExecOptions<'_>,
) -> anyhow::Result<ExitStatus> {
    let cache_dir = CacheDir::new(workspace, profile);

    let config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
        .with_context(|| format!("failed to load config `{}`", config_path.display()))?;

    let container = ContainerName::new(workspace, profile);
    let image_tag = container.as_image_tag();

    // Check if already running
    if docker::inspect_running(container.as_str()).await? {
        anyhow::bail!(
            "container `{}` is already running; use `dcc join` to reattach",
            container.as_str()
        );
    }

    // Ensure cache directory exists, then create any cache subdirectories
    // referenced as bind-mount sources (e.g. ${localCacheFolder}/node_modules).
    // Docker requires bind-mount source paths to exist on the host before startup.
    cache_dir.ensure_exists()?;

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
        format!("devcontainer.local_folder={}", workspace.root.display()),
    ]);
    args.extend([
        "--label".into(),
        format!("devcontainer.config_file={}", config_path.display()),
    ]);
    args.push("--rm".into());
    args.push("-dit".into());
    args.extend(["--workdir".into(), CONTAINER_WORKSPACE.into()]);
    args.extend(["--memory".into(), opts.limits.memory.to_owned()]);
    args.extend(["--cpus".into(), opts.limits.cpus.to_owned()]);

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
        dbg.push(format!(
            "user: {}   memory: {}   cpus: {}   workdir: {CONTAINER_WORKSPACE}",
            config.container_user, opts.limits.memory, opts.limits.cpus
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

    // initializeCommand runs on the host before the container is created/started.
    if let Some(cmd) = &config.initialize_command {
        if opts.skip_lifecycle {
            eprintln!("warning: skipping initializeCommand (--skip-lifecycle)");
        } else {
            let cmd = cmd
                .try_substitute(&|s| config::vars::resolve_container_env(s, &container_env))
                .context("initializeCommand")?;
            lifecycle::run_on_host(&cmd, &workspace.root)
                .await
                .context("initializeCommand failed")?;
        }
    }

    // Start the keep-alive container in the background; the user command runs in the
    // foreground via `docker exec` once the container and lifecycle hooks are ready.
    docker::start_detached(&args)
        .await
        .with_context(|| format!("failed to start container `{}`", container.as_str()))?;

    // Poll until the container is running before binding forwarder ports.
    wait_for_running(container.as_str())
        .await
        .with_context(|| format!("container `{}` failed to start", container.as_str()))?;

    // Run container-side lifecycle hooks (onCreateCommand through
    // postAttachCommand), in spec order, before forwarding ports or attaching.
    exec_lifecycle_hooks(
        container.as_str(),
        &config,
        &feature_runtime,
        &local_workspace,
        &local_cache,
        &container_env,
        opts.skip_lifecycle,
    )
    .await?;

    // Bind a host-side TCP relay on 127.0.0.1 for each forwarded port.
    let relay_handles = forward::forward_ports(container.as_str(), &config.forward_ports)
        .await
        .with_context(|| {
            format!(
                "failed to set up port forwarding for container `{}`",
                container.as_str()
            )
        })?;

    // Run the user command in the foreground via `docker exec`. Unlike attaching to
    // PID 1, this works for both one-off commands (`ls`) and interactive shells
    // (`bash`): output streams live and the command's real exit code is returned.
    let status = docker::exec_foreground(
        container.as_str(),
        &config.container_user,
        CONTAINER_WORKSPACE,
        &override_args,
        tty,
    )
    .await
    .with_context(|| {
        format!(
            "failed to run command in container `{}`",
            container.as_str()
        )
    })?;

    // Tear down port forwarders, then stop the keep-alive container (`--rm` removes it).
    for handle in relay_handles {
        handle.abort();
    }
    docker::stop_container(container.as_str())
        .await
        .with_context(|| format!("failed to stop container `{}`", container.as_str()))?;

    Ok(status)
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

/// Runs the container-side lifecycle hooks (`onCreateCommand` through
/// `postAttachCommand`) in spec order. For each hook type, feature-contributed
/// hooks run first, in feature installation order, followed by the
/// devcontainer.json hook of that type. A non-zero exit from any hook aborts
/// immediately, skipping subsequent hooks.
///
/// When `skip_lifecycle` is set, no hook runs; instead a warning naming each one is
/// printed, so a misbehaving hook can be bypassed for debugging.
async fn exec_lifecycle_hooks(
    container: &str,
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    local_workspace: &str,
    local_cache: &str,
    container_env: &std::collections::HashMap<String, String>,
    skip_lifecycle: bool,
) -> anyhow::Result<()> {
    if skip_lifecycle {
        for warning in skipped_hook_warnings(config, feature_runtime) {
            eprintln!("warning: {warning}");
        }
        return Ok(());
    }

    // Feature hooks need host/localEnv substitution; `${containerEnv:…}` is then
    // resolved for both feature and devcontainer.json hooks. devcontainer.json
    // hooks were already host-substituted at config-load (containerEnv deferred).
    let substitute = |s: &str| -> anyhow::Result<String> {
        let s = config::vars::apply_substitution(s, local_workspace, local_cache);
        config::vars::resolve_container_env(&s, container_env)
    };
    let resolve_cenv = |s: &str| config::vars::resolve_container_env(s, container_env);

    for (name, get) in lifecycle::HOOKS {
        for (feature_id, hooks) in &feature_runtime.feature_hooks {
            if let Some(cmd) = get(hooks) {
                let cmd = cmd
                    .try_substitute(&substitute)
                    .with_context(|| format!("{name} from feature `{feature_id}`"))?;
                lifecycle::run_in_container(
                    &cmd,
                    container,
                    &config.container_user,
                    CONTAINER_WORKSPACE,
                )
                .await
                .with_context(|| format!("{name} from feature `{feature_id}` failed"))?;
            }
        }

        if let Some(cmd) = get(&config.lifecycle) {
            let cmd = cmd
                .try_substitute(&resolve_cenv)
                .with_context(|| name.to_string())?;
            lifecycle::run_in_container(
                &cmd,
                container,
                &config.container_user,
                CONTAINER_WORKSPACE,
            )
            .await
            .with_context(|| format!("{name} failed"))?;
        }
    }

    Ok(())
}

/// Builds the warning messages for lifecycle hooks skipped under `--skip-lifecycle`,
/// in the same spec execution order they would otherwise run: for each hook
/// type, feature-contributed hooks (in installation order) first, then the
/// devcontainer.json hook. Only hooks that are actually present are listed.
fn skipped_hook_warnings(
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
) -> Vec<String> {
    let mut warnings = Vec::new();
    for (name, get) in lifecycle::HOOKS {
        for (feature_id, hooks) in &feature_runtime.feature_hooks {
            if get(hooks).is_some() {
                warnings.push(format!(
                    "skipping {name} from feature `{feature_id}` (--skip-lifecycle)"
                ));
            }
        }
        if get(&config.lifecycle).is_some() {
            warnings.push(format!("skipping {name} (--skip-lifecycle)"));
        }
    }
    warnings
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
    if config.remote_env.values().any(|s| has(s))
        || feature_runtime.remote_env.values().any(|s| has(s))
    {
        return true;
    }
    // Lifecycle commands: host initializeCommand plus the in-container hooks from both
    // devcontainer.json and features.
    let mut cmds: Vec<&lifecycle::LifecycleCommand> = config.initialize_command.iter().collect();
    for (_name, get) in lifecycle::HOOKS {
        cmds.extend(get(&config.lifecycle));
        for (_id, hooks) in &feature_runtime.feature_hooks {
            cmds.extend(get(hooks));
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

/// Builds the `--debug` lifecycle listing in execution order: `initializeCommand`
/// (host) first, then for each hook type the feature-contributed hooks (in
/// installation order) followed by the devcontainer.json hook. Each present
/// command is annotated when `skip_lifecycle` is set.
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
    for (name, get) in lifecycle::HOOKS {
        for (feature_id, hooks) in &feature_runtime.feature_hooks {
            if let Some(cmd) = get(hooks) {
                lines.push(format!(
                    "  {name} (feature {feature_id}): {}{suffix}",
                    describe_lifecycle_command(cmd)
                ));
            }
        }
        if let Some(cmd) = get(&config.lifecycle) {
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
            image: "img".into(),
            features: IndexMap::new(),
            container_env: HashMap::new(),
            remote_env: HashMap::new(),
            container_user: "dev".into(),
            mounts: Vec::new(),
            forward_ports: Vec::new(),
            initialize_command: None,
            lifecycle: LifecycleHooks::default(),
            scripts: HashMap::new(),
        }
    }

    fn shell(s: &str) -> Option<LifecycleCommand> {
        Some(LifecycleCommand::Shell(s.to_string()))
    }

    #[test]
    fn skipped_hook_warnings_empty_when_no_hooks() {
        let config = empty_config();
        let runtime = FeatureRuntimeConfig::default();
        assert!(skipped_hook_warnings(&config, &runtime).is_empty());
    }

    #[test]
    fn skipped_hook_warnings_lists_devcontainer_hooks_in_spec_order() {
        let mut config = empty_config();
        // Set out of spec order; output must still follow HOOKS order.
        config.lifecycle.post_attach_command = shell("echo attach");
        config.lifecycle.on_create_command = shell("echo create");
        let runtime = FeatureRuntimeConfig::default();
        assert_eq!(
            skipped_hook_warnings(&config, &runtime),
            vec![
                "skipping onCreateCommand (--skip-lifecycle)".to_string(),
                "skipping postAttachCommand (--skip-lifecycle)".to_string(),
            ]
        );
    }

    #[test]
    fn skipped_hook_warnings_feature_hook_named_and_ordered_before_devcontainer() {
        let mut config = empty_config();
        config.lifecycle.post_create_command = shell("echo dc");
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.feature_hooks.push((
            "node".to_string(),
            LifecycleHooks {
                post_create_command: shell("echo feat"),
                ..Default::default()
            },
        ));
        assert_eq!(
            skipped_hook_warnings(&config, &runtime),
            vec![
                "skipping postCreateCommand from feature `node` (--skip-lifecycle)".to_string(),
                "skipping postCreateCommand (--skip-lifecycle)".to_string(),
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
        config.lifecycle.post_create_command = shell("cargo fetch");
        let mut runtime = FeatureRuntimeConfig::default();
        runtime.feature_hooks.push((
            "node".to_string(),
            LifecycleHooks {
                post_create_command: shell("npm ci"),
                ..Default::default()
            },
        ));
        assert_eq!(
            debug_lifecycle_lines(&config, &runtime, false),
            vec![
                "  initializeCommand (host): echo init".to_string(),
                "  postCreateCommand (feature node): npm ci".to_string(),
                "  postCreateCommand: cargo fetch".to_string(),
            ]
        );
    }

    #[test]
    fn debug_lifecycle_lines_annotates_skip() {
        let mut config = empty_config();
        config.lifecycle.on_create_command = shell("x");
        assert_eq!(
            debug_lifecycle_lines(&config, &FeatureRuntimeConfig::default(), true),
            vec!["  onCreateCommand: x  (skipped: --skip-lifecycle)".to_string()]
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
        config.lifecycle.post_create_command = shell("echo ${containerEnv:HOME}");
        assert!(references_container_env(
            &[],
            &config,
            &FeatureRuntimeConfig::default()
        ));
    }
}
