use std::path::Path;

use anyhow::Context as _;
use serde::Serialize;

use crate::{cli::OutputFormat, profile::ProfileName};

#[derive(Debug, Serialize)]
pub(crate) struct DryRunReport {
    status: &'static str,
    command: String,
    profile: String,
    config: String,
    docker_invoked: bool,
    checks: Vec<&'static str>,
    skipped: Vec<&'static str>,
}

impl DryRunReport {
    pub(crate) fn new(
        command: impl Into<String>,
        profile: &ProfileName,
        config_path: &Path,
        checks: Vec<&'static str>,
        skipped: Vec<&'static str>,
    ) -> Self {
        Self {
            status: "ok",
            command: command.into(),
            profile: profile.as_str().to_string(),
            config: config_path.display().to_string(),
            docker_invoked: false,
            checks,
            skipped,
        }
    }

    pub(crate) fn print(&self, format: OutputFormat) -> anyhow::Result<()> {
        match format {
            OutputFormat::Text => {
                println!(
                    "dry-run ok: command={} profile={} config={} docker_invoked=false",
                    self.command, self.profile, self.config
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
