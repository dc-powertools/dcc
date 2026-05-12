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
    profile::{ContainerName, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn run(
    workspace: &Workspace,
    profile: &ProfileName,
    memory: &str,
    cpus: &str,
    override_args: &[String],
    strict: bool,
) -> anyhow::Result<ExitStatus> {
    let config_path = profile.config_path(workspace);
    let cache_dir = CacheDir::new(workspace, profile);

    let config = config::load_config(&config_path, workspace, &cache_dir, strict)
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
    ensure_cache_mount_sources(&config.mounts, &cache_dir)?;

    // Build the docker run argument list
    let mut args: Vec<String> = Vec::new();

    args.extend(["--name".into(), container.as_str().to_owned()]);
    args.push("--rm".into());
    args.push("-it".into());
    args.extend(["--memory".into(), memory.to_owned()]);
    args.extend(["--cpus".into(), cpus.to_owned()]);

    // containerEnv
    for (k, v) in &config.container_env {
        args.push("-e".into());
        args.push(format!("{k}={v}"));
    }

    // containerUser (omitted when not set — Docker uses the image's USER directive)
    if let Some(user) = &config.container_user {
        args.extend(["-u".into(), user.clone()]);
    }

    // forwardPorts (host port == container port)
    for port in &config.forward_ports {
        args.push("-p".into());
        args.push(format!("{port}:{port}"));
    }

    // mounts from config (string form: "type=bind,src=...,dst=...")
    for mount in &config.mounts {
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

    // Entrypoint resolution
    let (ep_flag, post_image_args) =
        resolve_entrypoint(override_args, config.entrypoint.as_deref());
    if let Some(ep) = ep_flag {
        args.extend(["--entrypoint".into(), ep]);
    }

    // Image tag (must come after all flags)
    args.push(image_tag.as_str().to_owned());

    // Post-image arguments
    args.extend(post_image_args);

    docker::run_container(&args).await
}

/// Creates host-side source directories for bind mounts whose source lives under the cache dir.
///
/// Restricted to the cache directory because that is dcc-managed space. Creating arbitrary
/// host paths would silently mask misconfigurations (e.g. a typo pointing at ~/.ssh).
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

/// Determines (entrypoint_flag, post_image_args) from override_args and configured entrypoint.
fn resolve_entrypoint(
    override_args: &[String],
    configured: Option<&[String]>,
) -> (Option<String>, Vec<String>) {
    let effective = if !override_args.is_empty() {
        override_args
    } else {
        match configured {
            Some(ep) if !ep.is_empty() => ep,
            _ => return (None, Vec::new()),
        }
    };
    (Some(effective[0].clone()), effective[1..].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{profile::ProfileName, workspace::Workspace};

    fn s(x: &str) -> String {
        x.to_string()
    }
    fn sv(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

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
        ensure_cache_mount_sources(&[mount.clone()], &cache).unwrap();
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

    #[test]
    fn override_args_take_first_as_entrypoint() {
        let (ep, rest) = resolve_entrypoint(&sv(&["npm", "serve"]), None);
        assert_eq!(ep, Some(s("npm")));
        assert_eq!(rest, sv(&["serve"]));
    }

    #[test]
    fn override_single_arg() {
        let (ep, rest) = resolve_entrypoint(&sv(&["bash"]), None);
        assert_eq!(ep, Some(s("bash")));
        assert_eq!(rest, sv(&[]));
    }

    #[test]
    fn configured_entrypoint_used_when_no_override() {
        let ep_vec = sv(&["bash", "-c", "script.sh"]);
        let (ep, rest) = resolve_entrypoint(&[], Some(&ep_vec));
        assert_eq!(ep, Some(s("bash")));
        assert_eq!(rest, sv(&["-c", "script.sh"]));
    }

    #[test]
    fn no_entrypoint_configured_or_overridden() {
        let (ep, rest) = resolve_entrypoint(&[], None);
        assert_eq!(ep, None);
        assert_eq!(rest, sv(&[]));
    }

    #[test]
    fn override_takes_precedence_over_configured() {
        let configured = sv(&["/bin/sh"]);
        let override_args = sv(&["bash"]);
        let (ep, _) = resolve_entrypoint(&override_args, Some(&configured));
        assert_eq!(ep, Some(s("bash")));
    }

    #[test]
    fn empty_configured_entrypoint_treated_as_none() {
        let configured = sv(&[]);
        let (ep, rest) = resolve_entrypoint(&[], Some(&configured));
        assert_eq!(ep, None);
        assert_eq!(rest, sv(&[]));
    }
}
