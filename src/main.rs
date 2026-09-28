mod build;
mod cache;
mod cli;
mod config;
mod docker;
mod dry_run;
mod exec;
mod feature;
mod features;
mod forward;
mod lifecycle;
mod profile;
mod run;
mod runtime;
mod runtime_snapshot;
mod seed;
mod stop;
mod supervisor;
mod uid;
mod version;
mod workspace;

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use clap::{CommandFactory as _, FromArgMatches as _};

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
    let matches = cli::Cli::command().get_matches();
    let cli = cli::Cli::from_arg_matches(&matches)?;
    let (memory_explicit, cpus_explicit) = match matches.subcommand() {
        Some(("exec" | "start" | "attach" | "run", args)) => (
            args.value_source("memory") == Some(clap::parser::ValueSource::CommandLine),
            args.value_source("cpus") == Some(clap::parser::ValueSource::CommandLine),
        ),
        _ => (false, false),
    };
    if let cli::Command::Feature {
        command:
            Some(cli::FeatureCommand::Validate {
                path,
                upstream_only,
            }),
        add,
        remove,
    } = &cli.command
    {
        // Clap's args_conflicts_with_subcommands also rejects global flags before
        // `validate`. Limit this conflict to the two profile-editing operations.
        if !add.is_empty() || !remove.is_empty() {
            cli::Cli::command()
                .error(
                    clap::error::ErrorKind::ArgumentConflict,
                    "feature validate cannot be combined with --add or --remove",
                )
                .exit();
        }
        return features::validate::validate_path(path, *upstream_only, cli.format);
    }
    if let cli::Command::Profile {
        command:
            cli::ProfileCommand::Bootstrap {
                image,
                dockerfile,
                extends,
            },
    } = &cli.command
    {
        let workspace = workspace::find_bootstrap_workspace()
            .context("failed to locate workspace for profile bootstrap")?;
        return profile::bootstrap_profile(
            &workspace,
            &cli.profile,
            profile::BootstrapOptions {
                image: image.as_deref(),
                dockerfile: dockerfile.as_deref(),
                extends: extends.as_deref(),
                strict: cli.strict,
                dry_run: cli.dry_run,
                debug: cli.debug,
                format: cli.format,
            },
        );
    }

    let workspace =
        workspace::find_workspace().context("failed to locate .devcontainer directory")?;
    if let cli::Command::Profile {
        command: cli::ProfileCommand::List,
    } = &cli.command
    {
        return profile::list_profiles(&workspace, cli.format, cli.dry_run, cli.debug).await;
    }

    let cwd = std::env::current_dir().context("failed to determine current working directory")?;
    let (profile, config_path) = resolve_profile(&cli.profile, &workspace, &cwd)?;
    match cli.command {
        cli::Command::Build {
            no_cache,
            refresh_only,
            allow_unsafe_runtime,
            reseed_state,
        } => {
            build::build(
                &workspace,
                &profile,
                &config_path,
                build::BuildOptions {
                    no_cache,
                    refresh_only,
                    strict: cli.strict,
                    allow_unsafe_runtime,
                    reseed_state,
                    dry_run: cli.dry_run,
                    debug: cli.debug,
                    format: cli.format,
                },
            )
            .await
        }
        cli::Command::Exec {
            memory,
            cpus,
            skip_lifecycle,
            allow_unsafe_runtime,
            keep,
            args,
        } => {
            let status = exec::exec(
                &workspace,
                &profile,
                &config_path,
                &args,
                exec::ExecOptions {
                    limits: exec::ResourceLimits {
                        memory_explicit,
                        cpus_explicit,
                        memory: &memory,
                        cpus: &cpus,
                    },
                    skip_lifecycle,
                    debug: cli.debug,
                    strict: cli.strict,
                    profile_arg: &cli.profile,
                    allow_unsafe_runtime,
                    keep,
                    dry_run: cli.dry_run,
                    format: cli.format,
                },
            )
            .await?;
            std::process::exit(status.code().unwrap_or(1));
        }
        cli::Command::Start {
            memory,
            cpus,
            allow_unsafe_runtime,
        } => {
            exec::start(
                &workspace,
                &profile,
                &config_path,
                exec::ExecOptions {
                    limits: exec::ResourceLimits {
                        memory_explicit,
                        cpus_explicit,
                        memory: &memory,
                        cpus: &cpus,
                    },
                    skip_lifecycle: false,
                    debug: cli.debug,
                    strict: cli.strict,
                    profile_arg: &cli.profile,
                    allow_unsafe_runtime,
                    keep: true,
                    dry_run: cli.dry_run,
                    format: cli.format,
                },
            )
            .await
        }
        cli::Command::Attach {
            memory,
            cpus,
            allow_unsafe_runtime,
            keep,
            args,
        } => {
            let status = exec::attach(
                &workspace,
                &profile,
                &config_path,
                &args,
                exec::ExecOptions {
                    limits: exec::ResourceLimits {
                        memory_explicit,
                        cpus_explicit,
                        memory: &memory,
                        cpus: &cpus,
                    },
                    skip_lifecycle: false,
                    debug: cli.debug,
                    strict: cli.strict,
                    profile_arg: &cli.profile,
                    allow_unsafe_runtime,
                    keep,
                    dry_run: cli.dry_run,
                    format: cli.format,
                },
            )
            .await?;
            std::process::exit(status.code().unwrap_or(1));
        }
        cli::Command::Stop { now, kill } => {
            stop::stop(
                &workspace,
                &profile,
                &config_path,
                stop::StopOptions {
                    strict: cli.strict,
                    profile_arg: &cli.profile,
                    dry_run: cli.dry_run,
                    debug: cli.debug,
                    format: cli.format,
                    now,
                    kill,
                },
            )
            .await
        }
        cli::Command::Id {} => {
            let container_id = profile::ContainerId::new(&workspace, &profile);
            if cli.debug {
                eprintln!("dcc debug: profile `{}`", profile.as_str());
                eprintln!("dcc debug: config `{}`", config_path.display());
                eprintln!("dcc debug: container id `{}`", container_id.as_str());
                eprintln!(
                    "dcc debug: image tag `{}`",
                    container_id.as_image_tag().as_str()
                );
            }
            if cli.format == cli::OutputFormat::Json {
                println!(
                    "{}",
                    serde_json::json!({
                        "profile": profile.as_str(),
                        "container_id": container_id.as_str()
                    })
                );
            } else {
                println!("{container_id}");
            }
            Ok(())
        }
        cli::Command::Profile {
            command: cli::ProfileCommand::List,
        } => profile::list_profiles(&workspace, cli.format, cli.dry_run, cli.debug).await,
        cli::Command::Profile {
            command: cli::ProfileCommand::Bootstrap { .. },
        } => anyhow::bail!("profile bootstrap was not dispatched before profile resolution"),
        cli::Command::Feature {
            command: None,
            add,
            remove,
        } => feature::update_features(
            &workspace,
            &profile,
            &config_path,
            feature::FeatureOptions {
                add,
                remove,
                strict: cli.strict,
                dry_run: cli.dry_run,
                debug: cli.debug,
                format: cli.format,
            },
        ),
        cli::Command::Feature {
            command: Some(_), ..
        } => {
            anyhow::bail!("feature validation was not dispatched before workspace resolution")
        }
        cli::Command::Run {
            memory,
            cpus,
            allow_unsafe_runtime,
            keep,
            script,
        } => {
            run::run(
                &workspace,
                &profile,
                &config_path,
                script.as_deref(),
                exec::ExecOptions {
                    limits: exec::ResourceLimits {
                        memory_explicit,
                        cpus_explicit,
                        memory: &memory,
                        cpus: &cpus,
                    },
                    skip_lifecycle: false,
                    debug: cli.debug,
                    strict: cli.strict,
                    profile_arg: &cli.profile,
                    allow_unsafe_runtime,
                    keep,
                    dry_run: cli.dry_run,
                    format: cli.format,
                },
            )
            .await
        }
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
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    if let (Some(parent), Some(name)) = (raw.parent(), raw.file_name()) {
                        return std::fs::canonicalize(parent).map(|parent| parent.join(name));
                    }
                }
                Err(e)
            })
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
