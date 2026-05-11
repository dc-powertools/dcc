use anyhow::Context as _;

use crate::{
    docker,
    profile::{ContainerName, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn join(workspace: &Workspace, profile: &ProfileName) -> anyhow::Result<()> {
    let container = ContainerName::new(workspace, profile);
    let status = docker::attach(container.as_str())
        .await
        .with_context(|| format!("failed to attach to container `{}`", container.as_str()))?;

    if !status.success() {
        anyhow::bail!(
            "could not attach to container `{}`; \
             make sure it is running (try `dcc run` first)",
            container.as_str()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // The attach path requires a live Docker daemon.
    // docker::attach and is_not_running_error have their own unit tests.
}
