use std::path::Path;

use anyhow::Context as _;
use serde::Serialize;

use crate::{
    cli::OutputFormat,
    profile::{ContainerId, ProfileName},
    workspace::Workspace,
};

#[derive(Debug, Serialize)]
pub(crate) struct DryRunReport {
    status: &'static str,
    command: String,
    profile: String,
    container_id: String,
    config: String,
    docker_invoked: bool,
    checks: Vec<String>,
    skipped: Vec<String>,
}

impl DryRunReport {
    pub(crate) fn new(
        command: impl Into<String>,
        workspace: &Workspace,
        profile: &ProfileName,
        config_path: &Path,
        checks: impl IntoIterator<Item = impl Into<String>>,
        skipped: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            status: "ok",
            command: command.into(),
            profile: profile.as_str().to_string(),
            container_id: ContainerId::new(workspace, profile).as_str().to_string(),
            config: config_path.display().to_string(),
            docker_invoked: false,
            checks: checks.into_iter().map(Into::into).collect(),
            skipped: skipped.into_iter().map(Into::into).collect(),
        }
    }

    pub(crate) fn print(&self, format: OutputFormat) -> anyhow::Result<()> {
        match format {
            OutputFormat::Text => {
                println!(
                    "dry-run ok: command={} profile={} container_id={} config={} docker_invoked=false",
                    self.command, self.profile, self.container_id, self.config
                );
                if !self.skipped.is_empty() {
                    println!("skipped: {}", self.skipped.join(", "));
                }
                Ok(())
            }
            OutputFormat::Json => {
                let json = serde_json::to_string_pretty(self)
                    .context("failed to serialize dry-run report")?;
                println!("{json}");
                Ok(())
            }
        }
    }
}
