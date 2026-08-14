use std::path::Path;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config, docker, dry_run,
    profile::{ContainerId, ProfileName},
    supervisor, version,
    workspace::Workspace,
};

pub(crate) async fn stop(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    opts: StopOptions<'_>,
) -> anyhow::Result<()> {
    let container_id = ContainerId::new(workspace, profile);
    if opts.debug {
        eprintln!("dcc debug: command `stop`");
        eprintln!("dcc debug: profile `{}`", profile.as_str());
        eprintln!("dcc debug: config `{}`", config_path.display());
        eprintln!("dcc debug: container id `{}`", container_id.as_str());
        eprintln!(
            "dcc debug: image tag `{}`",
            container_id.as_image_tag().as_str()
        );
    }
    let variant = if opts.kill {
        "kill"
    } else if opts.now {
        "now"
    } else {
        "graceful"
    };
    if opts.dry_run {
        let cache_dir = CacheDir::new(workspace, profile);
        let _config = config::load_config(config_path, workspace, &cache_dir, opts.strict)
            .with_context(|| format!("failed to load config `{}`", config_path.display()))?;
        let action = match variant {
            "kill" => "docker kill",
            "now" => "dcc-ctl stop-now",
            _ => "dcc-ctl stop (graceful drain)",
        };
        return dry_run::DryRunReport::new(
            "stop",
            profile,
            config_path,
            vec!["workspace resolved", "profile resolved", "config loaded"],
            vec![
                "docker image version inspection",
                "docker container lookup",
                action,
            ],
        )
        .print(opts.format);
    }
    version::warn_if_image_version_mismatch_best_effort(
        container_id.as_image_tag().as_str(),
        opts.profile_arg,
        opts.strict,
    )
    .await;

    // If no container is running, all variants are idempotent successes.
    let container = match docker::running_container_name_by_id(container_id.as_str()).await? {
        Some(name) => name,
        None => {
            if opts.debug {
                eprintln!(
                    "dcc debug: no running container for `{}`",
                    container_id.as_str()
                );
            }
            return Ok(());
        }
    };

    if opts.debug {
        eprintln!("dcc debug: stopping container `{container}` ({variant})");
    }

    match variant {
        "kill" => {
            docker::kill_container(&container)
                .await
                .with_context(|| format!("failed to kill container `{container}`"))?;
        }
        "now" => {
            // Signal the supervisor to terminate running commands and run shutdown hooks.
            let status = docker::exec(
                &container,
                "root",
                "/",
                &[
                    format!("{}/dcc-ctl", supervisor::RT_MOUNT),
                    "stop-now".to_string(),
                ],
            )
            .await
            .with_context(|| format!("failed to signal stop-now on container `{container}`"))?;
            if !status.success() {
                anyhow::bail!(
                    "`dcc-ctl stop-now` exited with status {} on container `{container}`",
                    status.code().unwrap_or(-1)
                );
            }
            wait_for_removed(&container).await;
        }
        _ => {
            // Graceful: signal the supervisor to stop accepting new commands and drain.
            let signaled = docker::exec(
                &container,
                "root",
                "/",
                &[
                    format!("{}/dcc-ctl", supervisor::RT_MOUNT),
                    "stop".to_string(),
                ],
            )
            .await;
            match signaled {
                Ok(status) if status.success() => {
                    wait_for_removed(&container).await;
                }
                Ok(status) => {
                    // The control script ran but failed. Fall back to docker stop.
                    docker::stop_container(&container).await.with_context(|| {
                        format!(
                            "failed to stop container `{container}` (dcc-ctl exit {}: \
                             try `dcc stop --kill`)",
                            status.code().unwrap_or(-1)
                        )
                    })?;
                }
                Err(e) => {
                    // The supervisor is unreachable (wedged or corrupted). Fall back to
                    // docker stop and point the user at --kill for a forceful teardown.
                    docker::stop_container(&container).await.with_context(|| {
                        format!(
                            "failed to stop container `{container}` (supervisor unreachable; \
                             try `dcc stop --kill`): {e:#}"
                        )
                    })?;
                }
            }
        }
    }
    Ok(())
}

/// Wait for a container to be removed after a graceful/now stop. Best-effort: if the
/// container does not disappear within the timeout, return without error (the caller
/// may need `dcc stop --kill` for a wedged container).
async fn wait_for_removed(container: &str) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        if let Ok(false) = docker::inspect_running(container).await {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

pub(crate) struct StopOptions<'a> {
    pub(crate) strict: bool,
    pub(crate) profile_arg: &'a str,
    pub(crate) dry_run: bool,
    pub(crate) debug: bool,
    pub(crate) format: crate::cli::OutputFormat,
    pub(crate) now: bool,
    pub(crate) kill: bool,
}

#[cfg(test)]
mod tests {
    // docker stop/kill handle idempotency; integration tests cover the full path.
    // The is_not_running_error helper in docker.rs has its own unit tests.
}
