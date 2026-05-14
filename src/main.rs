mod build;
mod cache;
mod cli;
mod config;
mod docker;
mod features;
mod join;
mod profile;
mod run;
mod stop;
mod workspace;

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use clap::Parser as _;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    if let Err(e) = run().await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let cwd = std::env::current_dir().context("failed to determine current working directory")?;
    let workspace =
        workspace::find_workspace().context("failed to locate .devcontainer directory")?;
    let (profile, config_path) = resolve_profile(&cli.profile, &workspace, &cwd)?;

    match cli.command {
        cli::Command::Build { no_cache } => {
            build::build(&workspace, &profile, &config_path, no_cache, cli.strict).await
        }
        cli::Command::Run { memory, cpus, args } => {
            let status = run::run(
                &workspace,
                &profile,
                &config_path,
                &memory,
                &cpus,
                &args,
                cli.strict,
            )
            .await?;
            std::process::exit(status.code().unwrap_or(1));
        }
        cli::Command::Join => join::join(&workspace, &profile).await,
        cli::Command::Stop => stop::stop(&workspace, &profile).await,
    }
}

/// Returns true when `arg` should be interpreted as a file path rather than a
/// profile name. Matches the same prefix rules used by shells to distinguish
/// bare names from paths: leading `/`, `./`, or `../`.
fn is_path_arg(arg: &str) -> bool {
    arg.starts_with('/') || arg.starts_with("./") || arg.starts_with("../")
}

/// Resolves the `-p` / `--profile` argument to a `(ProfileName, config_path)` pair.
///
/// Named profiles (`-p claude`) map to `.devcontainer/claude.json` relative to the
/// workspace root. Path-based profiles (`-p ./configs/claude.json`) resolve the
/// given path relative to `cwd`, canonicalize it, and derive the profile name from
/// the path (relative to workspace root when inside, absolute otherwise).
fn resolve_profile(
    arg: &str,
    workspace: &workspace::Workspace,
    cwd: &Path,
) -> anyhow::Result<(profile::ProfileName, PathBuf)> {
    if is_path_arg(arg) {
        let raw = cwd.join(arg);
        let config_path = std::fs::canonicalize(&raw)
            .with_context(|| format!("failed to resolve config path `{}`", raw.display()))?;
        let name = profile::path_to_profile_name(&config_path, workspace);
        Ok((name, config_path))
    } else {
        let name = profile::ProfileName::new(arg);
        let config_path = name.config_path(workspace);
        Ok((name, config_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_path_arg_absolute() {
        assert!(is_path_arg("/home/user/config.json"));
    }

    #[test]
    fn is_path_arg_dot_slash() {
        assert!(is_path_arg("./config.json"));
        assert!(is_path_arg("./nested/config.json"));
    }

    #[test]
    fn is_path_arg_dot_dot_slash() {
        assert!(is_path_arg("../sibling/config.json"));
    }

    #[test]
    fn is_path_arg_bare_name() {
        assert!(!is_path_arg("claude"));
        assert!(!is_path_arg("devcontainer"));
        assert!(!is_path_arg("my-profile"));
    }

    #[test]
    fn is_path_arg_bare_dot_or_dotdot() {
        // "." and ".." without a trailing slash are not path args
        assert!(!is_path_arg("."));
        assert!(!is_path_arg(".."));
    }
}
