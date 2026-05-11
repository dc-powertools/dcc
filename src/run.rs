use std::process::ExitStatus;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config, docker,
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

    // Ensure cache directory exists
    cache_dir.ensure_exists().with_context(|| {
        format!(
            "failed to create cache directory `{}`",
            cache_dir.host_path.display()
        )
    })?;

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

    // containerUser
    args.extend(["-u".into(), config.container_user.clone()]);

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
    args.push(format!("{}:/workspace", workspace.root.display()));

    // cache bind mount
    args.push("-v".into());
    args.push(format!("{}:/cache", cache_dir.host_path.display()));

    // mask .dcc directory inside container
    args.extend(["--tmpfs".into(), "/workspace/.dcc".into()]);

    // Entrypoint resolution
    let (ep_flag, post_image_args) = resolve_entrypoint(override_args, config.entrypoint.as_ref());
    if let Some(ep) = ep_flag {
        args.extend(["--entrypoint".into(), ep]);
    }

    // Image tag (must come after all flags)
    args.push(image_tag.as_str().to_owned());

    // Post-image arguments
    args.extend(post_image_args);

    // Convert to &str for docker::run_container
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    docker::run_container(&arg_refs).await
}

/// Determines (entrypoint_flag, post_image_args) from override_args and configured entrypoint.
fn resolve_entrypoint(
    override_args: &[String],
    configured: Option<&Vec<String>>,
) -> (Option<String>, Vec<String>) {
    if !override_args.is_empty() {
        (Some(override_args[0].clone()), override_args[1..].to_vec())
    } else if let Some(ep) = configured {
        if !ep.is_empty() {
            (Some(ep[0].clone()), ep[1..].to_vec())
        } else {
            (None, Vec::new())
        }
    } else {
        (None, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> String {
        x.to_string()
    }
    fn sv(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
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
