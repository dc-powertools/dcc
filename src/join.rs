use anyhow::Context as _;

use crate::{
    docker,
    profile::{ContainerId, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn join(workspace: &Workspace, profile: &ProfileName) -> anyhow::Result<()> {
    let container_id = ContainerId::new(workspace, profile);
    let container = docker::running_container_name_by_id(container_id.as_str())
        .await?
        .unwrap_or_else(|| container_id.as_str().to_string());
    let status = docker::attach(&container)
        .await
        .with_context(|| format!("failed to attach to container `{container}`"))?;

    if !status.success() {
        anyhow::bail!(
            "could not attach to container `{}`; \
             make sure it is running (try `dcc run` first)",
            container
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // The attach path requires a live Docker daemon.
    // docker::attach and is_not_running_error have their own unit tests.
}
