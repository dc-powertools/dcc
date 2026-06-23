use std::path::Path;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config, docker,
    profile::{ContainerId, ProfileName},
    version,
    workspace::Workspace,
};

pub(crate) async fn stop(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    strict: bool,
    profile_arg: &str,
) -> anyhow::Result<()> {
    let container_id = ContainerId::new(workspace, profile);
    let current_uses_fast_path = current_uses_fast_path(workspace, profile, config_path, strict);
    version::warn_if_image_version_mismatch_best_effort(
        container_id.as_image_tag().as_str(),
        current_uses_fast_path,
        profile_arg,
        strict,
    )
    .await;
    let container = docker::running_container_name_by_id(container_id.as_str())
        .await?
        .unwrap_or_else(|| container_id.as_str().to_string());
    docker::stop_container(&container)
        .await
        .with_context(|| format!("failed to stop container `{container}`"))
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
