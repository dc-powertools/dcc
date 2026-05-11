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

pub(crate) async fn build(tag: &str, no_cache: bool, context: Vec<u8>) -> anyhow::Result<()> {
    let mut cmd = Command::new("docker");
    cmd.arg("build");
    if no_cache {
        cmd.arg("--no-cache");
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

pub(crate) async fn run_container(args: &[String]) -> anyhow::Result<ExitStatus> {
    Command::new("docker")
        .arg("run")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn `docker run`")?
        .wait()
        .await
        .context("failed to wait for `docker run`")
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

fn check_status(status: ExitStatus, cmd: &str) -> anyhow::Result<()> {
    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        anyhow::bail!("`{cmd}` exited with status {code}")
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
}
