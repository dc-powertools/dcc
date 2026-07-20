use std::path::Path;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config, docker, dry_run,
    profile::{ContainerId, ProfileName},
    runtime::RuntimeState,
    version,
    workspace::Workspace,
};

pub(crate) async fn stop(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    opts: StopOptions<'_>,
) -> anyhow::Result<()> {
    let container_id = ContainerId::new(workspace, profile);
    if opts.dry_run {
        let cache_dir = CacheDir::new(workspace, profile);
        let _config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
            .with_context(|| format!("failed to load config `{}`", config_path.display()))?;
        return dry_run::DryRunReport::new(
            "stop",
            profile,
            config_path,
            vec!["workspace resolved", "profile resolved", "config loaded"],
            vec![
                "docker image version inspection",
                "docker container lookup",
                "docker stop",
                "runtime state clearing",
            ],
        )
        .print(opts.format);
    }
    let current_uses_fast_path =
        current_uses_fast_path(workspace, profile, config_path, opts.strict);
    version::warn_if_image_version_mismatch_best_effort(
        container_id.as_image_tag().as_str(),
        current_uses_fast_path,
        opts.profile_arg,
        opts.strict,
    )
    .await;
    let container = docker::running_container_name_by_id(container_id.as_str())
        .await?
        .unwrap_or_else(|| container_id.as_str().to_string());
    docker::stop_container(&container)
        .await
        .with_context(|| format!("failed to stop container `{container}`"))?;
    RuntimeState::new(&CacheDir::new(workspace, profile)).clear()
}

pub(crate) struct StopOptions<'a> {
    pub(crate) strict: bool,
    pub(crate) profile_arg: &'a str,
    pub(crate) dry_run: bool,
    pub(crate) format: crate::cli::OutputFormat,
}

fn current_uses_fast_path(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    strict: bool,
) -> Option<bool> {
    let cache_dir = CacheDir::new(workspace, profile);
    let config = config::load_config(config_path, workspace, &cache_dir, strict).ok()?;
    Some(crate::build::uses_fast_path(&config))
}

#[cfg(test)]
mod tests {
    // docker::stop_container handles idempotency; integration tests cover the full path.
    // The is_not_running_error helper in docker.rs has its own unit tests.
}
