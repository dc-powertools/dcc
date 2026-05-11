use anyhow::Context as _;

use crate::{
    docker,
    profile::{ContainerName, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn stop(workspace: &Workspace, profile: &ProfileName) -> anyhow::Result<()> {
    let container = ContainerName::new(workspace, profile);
    docker::stop_container(container.as_str())
        .await
        .with_context(|| format!("failed to stop container `{}`", container.as_str()))
}

#[cfg(test)]
mod tests {
    // docker::stop_container handles idempotency; integration tests cover the full path.
    // The is_not_running_error helper in docker.rs has its own unit tests.
}
