use std::collections::HashMap;
use std::process::{ExitStatus, Stdio};

use anyhow::Context as _;
use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

pub(crate) async fn pull(image: &str) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["pull", image])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn `docker pull {image}`"))?
        .wait()
        .await
        .with_context(|| format!("failed to wait for `docker pull {image}`"))?;
    check_status(status, &format!("docker pull {image}"))
}

pub(crate) async fn tag(source: &str, target: &str) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .args(["tag", source, target])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn `docker tag {source} {target}`"))?
        .wait()
        .await
        .with_context(|| format!("failed to wait for `docker tag {source} {target}`"))?;
    check_status(status, &format!("docker tag {source} {target}"))
}

pub(crate) async fn build(
    tag: &str,
    no_cache: bool,
    context: Vec<u8>,
    metadata_label: Option<&str>,
) -> anyhow::Result<()> {
    let mut cmd = Command::new("docker");
    cmd.arg("build");
    if no_cache {
        cmd.arg("--no-cache");
    }
    if let Some(label) = metadata_label {
        cmd.args(["--label", &format!("devcontainer.metadata={label}")]);
    }
    cmd.args(["--tag", tag, "-"]);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn `docker build --tag {tag} -`"))?;

    // Write build context to stdin then close the pipe
    let mut stdin = child
        .stdin
        .take()
        // SAFETY: Stdio::piped() set above
        .expect("stdin was configured as piped");
    stdin
        .write_all(&context)
        .await
        .context("failed to write build context to docker stdin")?;
    drop(stdin); // closes pipe → docker build sees EOF

    let status = child
        .wait()
        .await
        .with_context(|| format!("failed to wait for `docker build --tag {tag} -`"))?;
    check_status(status, &format!("docker build --tag {tag} -"))
}

/// Starts a container detached (`docker run -d …`) and returns once Docker
/// confirms the container was created. The caller is responsible for running the
/// command (via [`exec_foreground`]), aborting port-forwarding tasks, and stopping
/// the container on exit.
pub(crate) async fn start_detached(args: &[String]) -> anyhow::Result<()> {
    // stderr is captured (not inherited) so that, on failure, Docker's own
    // diagnostic — e.g. "invalid mount config ... bind source path does not
    // exist" — is surfaced in the error instead of being discarded. On success
    // `docker run -d` prints only the container id to stdout, which we suppress.
    let output = Command::new("docker")
        .arg("run")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("failed to spawn `docker run`")?;
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(command_failure("docker run", code, &output.stderr));
    }
    Ok(())
}

/// Builds the argument list for a `docker exec` invocation. `interactive` adds
/// `-i` (keep stdin open) and `tty` adds `-t` (allocate a pseudo-TTY).
fn exec_args(
    container: &str,
    user: &str,
    workdir: &str,
    argv: &[String],
    interactive: bool,
    tty: bool,
) -> Vec<String> {
    let mut args = vec!["exec".to_string()];
    if interactive {
        args.push("-i".to_string());
    }
    if tty {
        args.push("-t".to_string());
    }
    args.extend([
        "-u".to_string(),
        user.to_string(),
        "-w".to_string(),
        workdir.to_string(),
        container.to_string(),
    ]);
    args.extend(argv.iter().cloned());
    args
}

/// Spawns `docker <args>` with stdio inherited and waits for it to finish.
async fn spawn_inherit(
    args: &[String],
    container: &str,
    argv: &[String],
) -> anyhow::Result<ExitStatus> {
    Command::new("docker")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| {
            format!(
                "failed to spawn `docker exec {container} {}`",
                argv.join(" ")
            )
        })?
        .wait()
        .await
        .with_context(|| {
            format!(
                "failed to wait for `docker exec {container} {}`",
                argv.join(" ")
            )
        })
}

/// Runs `argv` inside `container` as `user` from `workdir` via `docker exec`,
/// with stdio inherited. Non-interactive (no `-i`/`-t`): used for lifecycle hooks.
pub(crate) async fn exec(
    container: &str,
    user: &str,
    workdir: &str,
    argv: &[String],
) -> anyhow::Result<ExitStatus> {
    spawn_inherit(
        &exec_args(container, user, workdir, argv, false, false),
        container,
        argv,
    )
    .await
}

/// Runs `argv` inside `container` as `user` from `workdir` via an interactive
/// `docker exec -i` (adding `-t` when `tty` is set), with stdio inherited. Used for
/// the foreground command of `dcc exec`/`dcc run`, so both one-off commands (`ls`)
/// and interactive shells (`bash`) stream correctly and report the real exit code.
pub(crate) async fn exec_foreground(
    container: &str,
    user: &str,
    workdir: &str,
    argv: &[String],
    tty: bool,
) -> anyhow::Result<ExitStatus> {
    spawn_inherit(
        &exec_args(container, user, workdir, argv, true, tty),
        container,
        argv,
    )
    .await
}

pub(crate) async fn attach(container: &str) -> anyhow::Result<ExitStatus> {
    Command::new("docker")
        .args(["attach", container])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn `docker attach {container}`"))?
        .wait()
        .await
        .with_context(|| format!("failed to wait for `docker attach {container}`"))
}

pub(crate) async fn stop_container(container: &str) -> anyhow::Result<()> {
    let output = Command::new("docker")
        .args(["stop", container])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("failed to spawn `docker stop {container}`"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Idempotent: treat "not running" or "no such container" as success
    if is_not_running_error(&stderr) {
        return Ok(());
    }

    anyhow::bail!("`docker stop {container}` failed: {}", stderr.trim())
}

/// Reads the `devcontainer.metadata` label from a local Docker image.
/// Returns `None` when the image exists but the label is absent.
/// Returns `Err` when the image does not exist or the Docker daemon is unreachable.
pub(crate) async fn inspect_image_label(image: &str) -> anyhow::Result<Option<String>> {
    let output = Command::new("docker")
        .args([
            "image",
            "inspect",
            "--format",
            r#"{{index .Config.Labels "devcontainer.metadata"}}"#,
            image,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("failed to spawn `docker image inspect {image}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("`docker image inspect {image}` failed: {}", stderr.trim());
    }

    let value = String::from_utf8_lossy(&output.stdout);
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_owned()))
    }
}

/// Reads the environment baked into a local Docker image (`Config.Env`): the base
/// image's `ENV` plus every `containerEnv` directive `dcc build` baked in. Used to
/// resolve `${containerEnv:VAR}` references at run time. `remoteEnv` is *not*
/// part of the image, so it is intentionally absent from this map.
pub(crate) async fn inspect_image_env(image: &str) -> anyhow::Result<HashMap<String, String>> {
    let output = Command::new("docker")
        .args([
            "image",
            "inspect",
            "--format",
            "{{json .Config.Env}}",
            image,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("failed to spawn `docker image inspect {image}`"))?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(command_failure(
            &format!("docker image inspect {image}"),
            code,
            &output.stderr,
        ));
    }

    // `.Config.Env` serializes to `null` for an image with no env directives.
    let env: Option<Vec<String>> = serde_json::from_slice(&output.stdout)
        .with_context(|| format!("failed to parse env from `docker image inspect {image}`"))?;
    Ok(parse_env_list(env.unwrap_or_default()))
}

/// Splits a Docker `Config.Env` list (`KEY=VALUE` strings) into a map. Only the
/// first `=` separates key from value, so values may themselves contain `=`.
/// Entries with no `=` are skipped.
fn parse_env_list(env: Vec<String>) -> HashMap<String, String> {
    env.into_iter()
        .filter_map(|entry| {
            entry
                .split_once('=')
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
        })
        .collect()
}

pub(crate) async fn inspect_running(container: &str) -> anyhow::Result<bool> {
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Running}}", container])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null()) // suppress "No such object" when container doesn't exist
        .output()
        .await
        .with_context(|| format!("failed to spawn `docker inspect {container}`"))?;

    if !output.status.success() {
        // Container doesn't exist → not running
        return Ok(false);
    }

    let out = String::from_utf8_lossy(&output.stdout);
    Ok(out.trim() == "true")
}

fn is_not_running_error(stderr: &str) -> bool {
    stderr.contains("No such container") || stderr.contains("is not running")
}

pub(crate) fn check_status(status: ExitStatus, cmd: &str) -> anyhow::Result<()> {
    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        anyhow::bail!("`{cmd}` exited with status {code}")
    }
}

/// Builds an error for a failed command, appending its captured stderr when
/// present. Used by subprocess calls that pipe stderr (e.g. [`start_detached`])
/// so the underlying tool's diagnostic is not lost.
fn command_failure(cmd: &str, code: i32, stderr: &[u8]) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        anyhow::anyhow!("`{cmd}` exited with status {code}")
    } else {
        anyhow::anyhow!("`{cmd}` exited with status {code}: {stderr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_not_running_error_no_such_container() {
        assert!(is_not_running_error(
            "Error response from daemon: No such container: myapp"
        ));
    }

    #[test]
    fn is_not_running_error_not_running() {
        assert!(is_not_running_error(
            "Error response from daemon: Container abc123 is not running"
        ));
    }

    #[test]
    fn is_not_running_error_other_error() {
        assert!(!is_not_running_error(
            "Error response from daemon: context deadline exceeded"
        ));
    }

    #[test]
    fn is_not_running_error_empty() {
        assert!(!is_not_running_error(""));
    }

    #[test]
    fn command_failure_includes_trimmed_stderr() {
        let err = command_failure(
            "docker run",
            125,
            b"  docker: Error response from daemon: bind source path does not exist: /x\n",
        );
        let msg = err.to_string();
        assert!(msg.contains("exited with status 125"), "got: {msg}");
        assert!(
            msg.contains("bind source path does not exist: /x"),
            "got: {msg}"
        );
        assert!(!msg.contains('\n'), "stderr should be trimmed, got: {msg}");
    }

    #[test]
    fn command_failure_empty_stderr_falls_back_to_code() {
        let err = command_failure("docker run", 1, b"");
        assert_eq!(err.to_string(), "`docker run` exited with status 1");
    }

    #[test]
    fn command_failure_whitespace_only_stderr_falls_back_to_code() {
        let err = command_failure("docker run", 2, b"   \n  ");
        assert_eq!(err.to_string(), "`docker run` exited with status 2");
    }

    #[test]
    fn parse_env_list_splits_key_value() {
        let env = parse_env_list(vec!["PATH=/usr/bin:/bin".into(), "LANG=C.UTF-8".into()]);
        assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin:/bin"));
        assert_eq!(env.get("LANG").map(String::as_str), Some("C.UTF-8"));
    }

    #[test]
    fn parse_env_list_value_may_contain_equals() {
        let env = parse_env_list(vec!["FOO=a=b=c".into()]);
        assert_eq!(env.get("FOO").map(String::as_str), Some("a=b=c"));
    }

    #[test]
    fn parse_env_list_skips_entries_without_equals() {
        let env = parse_env_list(vec!["NOTANENV".into(), "OK=1".into()]);
        assert!(!env.contains_key("NOTANENV"));
        assert_eq!(env.get("OK").map(String::as_str), Some("1"));
    }

    #[test]
    fn parse_env_list_empty() {
        assert!(parse_env_list(vec![]).is_empty());
    }

    #[test]
    fn exec_args_non_interactive_has_no_i_or_t() {
        let argv = vec!["ls".to_string()];
        assert_eq!(
            exec_args("c", "dev", "/workspace", &argv, false, false),
            vec!["exec", "-u", "dev", "-w", "/workspace", "c", "ls"]
        );
    }

    #[test]
    fn exec_args_foreground_tty_has_i_and_t() {
        let argv = vec!["bash".to_string()];
        assert_eq!(
            exec_args("c", "dev", "/workspace", &argv, true, true),
            vec![
                "exec",
                "-i",
                "-t",
                "-u",
                "dev",
                "-w",
                "/workspace",
                "c",
                "bash"
            ]
        );
    }

    #[test]
    fn exec_args_foreground_no_tty_has_i_only() {
        let argv = vec!["ls".to_string(), "-la".to_string()];
        assert_eq!(
            exec_args("c", "dev", "/workspace", &argv, true, false),
            vec![
                "exec",
                "-i",
                "-u",
                "dev",
                "-w",
                "/workspace",
                "c",
                "ls",
                "-la"
            ]
        );
    }
}
