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

pub(crate) async fn run(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    memory: &str,
    cpus: &str,
    override_args: &[String],
    strict: bool,
) -> anyhow::Result<ExitStatus> {
    let cache_dir = CacheDir::new(workspace, profile);

    let config = config::load_config(config_path, workspace, &cache_dir, strict)
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

    // Apply variable substitution to feature mounts (same variables as devcontainer.json mounts)
    let local_workspace = workspace.root.to_string_lossy().into_owned();
    let local_cache = cache_dir.host_path.to_string_lossy().into_owned();
    let feature_mounts: Vec<String> = feature_runtime
        .mounts
        .iter()
        .map(|m| config::vars::apply_substitution(m, &local_workspace, &local_cache))
        .collect();

    // Combined mounts: feature mounts first, then devcontainer.json mounts
    let all_mounts: Vec<String> = feature_mounts
        .iter()
        .chain(config.mounts.iter())
        .cloned()
        .collect();

    ensure_cache_mount_sources(&all_mounts, &cache_dir)?;

    // Substitute local variables in feature remoteEnv templates
    let feature_remote_env: Vec<(String, String)> = feature_runtime
        .remote_env
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                config::vars::apply_substitution(v, &local_workspace, &local_cache),
            )
        })
        .collect();

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
    args.extend(["--memory".into(), memory.to_owned()]);
    args.extend(["--cpus".into(), cpus.to_owned()]);

    // containerUser (defaults to "dev" when not set in the devcontainer config)
    args.extend(["-u".into(), config.container_user.clone()]);

    // remoteEnv: passed as runtime flags (substitution already applied at config-load time)
    for (k, v) in &config.remote_env {
        args.push("-e".into());
        args.push(format!("{k}={v}"));
    }

    // feature remoteEnv: passed as runtime flags (substituted from templates above)
    for (k, v) in &feature_remote_env {
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

    // Entrypoint and post-image arguments come from the required CLI args.
    args.extend(["--entrypoint".into(), override_args[0].clone()]);

    // Image tag (must come after all flags)
    args.push(image_tag.as_str().to_owned());

    // Remaining CLI args become post-image arguments
    args.extend(override_args[1..].iter().cloned());

    // initializeCommand runs on the host before the container is created/started.
    if let Some(cmd) = &config.initialize_command {
        lifecycle::run_on_host(cmd, &workspace.root)
            .await
            .context("initializeCommand failed")?;
    }

    // Start the container in the background (-d); TTY is pre-allocated (-t)
    // so that `docker attach` below provides a proper interactive terminal.
    docker::start_detached(&args)
        .await
        .with_context(|| format!("failed to start container `{}`", container.as_str()))?;

    // Poll until the container is running before binding forwarder ports.
    wait_for_running(container.as_str())
        .await
        .with_context(|| format!("container `{}` failed to start", container.as_str()))?;

    // Run container-side lifecycle hooks (onCreateCommand through
    // postAttachCommand), in spec order, before forwarding ports or attaching.
    run_lifecycle_hooks(
        container.as_str(),
        &config,
        &feature_runtime,
        &local_workspace,
        &local_cache,
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

    // Attach to the container's interactive session; blocks until it exits or
    // the user detaches with the Docker escape sequence (Ctrl-P, Ctrl-Q).
    let status = docker::attach(container.as_str())
        .await
        .with_context(|| format!("failed to attach to container `{}`", container.as_str()))?;

    // Tear down port forwarders now that the container has exited or detached.
    for handle in relay_handles {
        handle.abort();
    }

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
async fn run_lifecycle_hooks(
    container: &str,
    config: &config::DevcontainerConfig,
    feature_runtime: &FeatureRuntimeConfig,
    local_workspace: &str,
    local_cache: &str,
) -> anyhow::Result<()> {
    let substitute = |s: &str| config::vars::apply_substitution(s, local_workspace, local_cache);

    for (name, get) in lifecycle::HOOKS {
        for (feature_id, hooks) in &feature_runtime.feature_hooks {
            if let Some(cmd) = get(hooks) {
                let cmd = cmd.substitute(&substitute);
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
            lifecycle::run_in_container(
                cmd,
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
}
