use anyhow::Context as _;

use crate::{
    docker,
    profile::{ContainerId, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn stop(workspace: &Workspace, profile: &ProfileName) -> anyhow::Result<()> {
    let container_id = ContainerId::new(workspace, profile);
    let container = docker::running_container_name_by_id(container_id.as_str())
        .await?
        .unwrap_or_else(|| container_id.as_str().to_string());
    docker::stop_container(&container)
        .await
        .with_context(|| format!("failed to stop container `{container}`"))
}

#[cfg(test)]
mod tests {
    // docker::stop_container handles idempotency; integration tests cover the full path.
    // The is_not_running_error helper in docker.rs has its own unit tests.
}
