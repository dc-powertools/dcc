use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};

use anyhow::Context as _;
use tokio::io::AsyncWriteExt as _;
use tokio::process::Command;

pub(crate) const CONTAINER_ID_LABEL: &str = "dcc.container_id";
pub(crate) const CONTAINER_ROLE_LABEL: &str = "dcc.container_role";
pub(crate) const CONTAINER_ROLE_RUNTIME: &str = "runtime";
pub(crate) const CONTAINER_ROLE_BUILD_PREP: &str = "build-prep";

const RUNNING_DCC_CONTAINERS_FORMAT: &str =
    r#"{{.Label "dcc.container_id"}}\t{{.Label "dcc.container_role"}}\t{{.Names}}"#;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RunningDccContainer {
    pub(crate) container_id: String,
    pub(crate) role: Option<String>,
    pub(crate) name: String,
}

pub(crate) async fn build(
    tag: &str,
    no_cache: bool,
    pull: bool,
    context: Vec<u8>,
    metadata_label: Option<&str>,
    seed_label: Option<&str>,
    build_args: Vec<(String, String)>,
) -> anyhow::Result<()> {
    let opts = DockerBuildOptions {
        tag: tag.to_string(),
        no_cache,
        pull,
        metadata_label: metadata_label.map(str::to_string),
        seed_label: seed_label.map(str::to_string),
        file: None,
        context_dir: None,
        build_args,
        target: None,
    };
    build_stdin(opts, context).await
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct DockerBuildOptions {
    pub(crate) tag: String,
    pub(crate) no_cache: bool,
    /// Pass `--pull` to `docker build` so `FROM` re-resolves the base image tag
    /// upstream rather than reusing a stale local image.
    pub(crate) pull: bool,
    pub(crate) metadata_label: Option<String>,
    /// Optional `dcc.seed` label value (resolved seed manifest JSON).
    pub(crate) seed_label: Option<String>,
    pub(crate) file: Option<PathBuf>,
    pub(crate) context_dir: Option<PathBuf>,
    pub(crate) build_args: Vec<(String, String)>,
    pub(crate) target: Option<String>,
}

pub(crate) fn build_args(opts: &DockerBuildOptions) -> Vec<String> {
    let mut args = vec!["build".to_string()];
    if opts.no_cache {
        args.push("--no-cache".to_string());
    }
    if opts.pull {
        args.push("--pull".to_string());
    }
    if let Some(label) = &opts.metadata_label {
        args.extend([
            "--label".to_string(),
            format!("devcontainer.metadata={label}"),
        ]);
    }
    if let Some(seed) = &opts.seed_label {
        args.extend(["--label".to_string(), format!("dcc.seed={seed}")]);
    }
    let mut build_args = opts.build_args.clone();
    build_args.sort_by(|a, b| a.0.cmp(&b.0));
    for (key, value) in build_args {
        args.extend(["--build-arg".to_string(), format!("{key}={value}")]);
    }
    if let Some(target) = &opts.target {
        args.extend(["--target".to_string(), target.clone()]);
    }
    args.extend(["--tag".to_string(), opts.tag.clone()]);
    if let Some(file) = &opts.file {
        args.extend(["--file".to_string(), file.display().to_string()]);
    }
    args.push(
        opts.context_dir
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "-".to_string()),
    );
    args
}

pub(crate) async fn build_path(opts: DockerBuildOptions) -> anyhow::Result<()> {
    let mut cmd = Command::new("docker");
    let args = build_args(&opts);
    cmd.args(&args);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let display = format!("docker {}", args.join(" "));
    let status = cmd
        .spawn()
        .with_context(|| format!("failed to spawn `{display}`"))?
        .wait()
        .await
        .with_context(|| format!("failed to wait for `{display}`"))?;
    check_status(status, &display)
}

async fn build_stdin(opts: DockerBuildOptions, context: Vec<u8>) -> anyhow::Result<()> {
    let mut cmd = Command::new("docker");
    let args = build_args(&opts);
    cmd.args(&args);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());
    let display = format!("docker {}", args.join(" "));

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn `{display}`"))?;

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
        .with_context(|| format!("failed to wait for `{display}`"))?;
    check_status(status, &display)
}

pub(crate) async fn image_exists(image: &str) -> anyhow::Result<bool> {
    let output = Command::new("docker")
        .args(["image", "inspect", image])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("failed to spawn `docker image inspect {image}`"))?;
    if output.status.success() {
        return Ok(true);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("No such image") || stderr.contains("No such object") {
        return Ok(false);
    }
    let code = output.status.code().unwrap_or(-1);
    Err(command_failure(
        &format!("docker image inspect {image}"),
        code,
        &output.stderr,
    ))
}

/// Runs a short-lived container to completion (`docker run --rm …`) with stdio
/// inherited, returning the exit status. Used for the state hydration container,
/// which copies image content to the mounted host state root and exits.
pub(crate) async fn run_to_completion(args: &[String]) -> anyhow::Result<()> {
    let status = Command::new("docker")
        .arg("run")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| "failed to spawn `docker run`")?
        .wait()
        .await
        .with_context(|| "failed to wait for `docker run`")?;
    check_status(status, "docker run")
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
async fn spawn_inherit(args: &[String], container: &str) -> anyhow::Result<ExitStatus> {
    Command::new("docker")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn docker exec for container {container}"))?
        .wait()
        .await
        .with_context(|| format!("failed to wait for docker exec for container {container}"))
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
    )
    .await
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

/// Unconditionally kill a container (`docker kill`). Emergency path for wedged or
/// corrupted containers. Idempotent: a missing container is treated as success.
pub(crate) async fn kill_container(container: &str) -> anyhow::Result<()> {
    let output = Command::new("docker")
        .args(["kill", container])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("failed to spawn `docker kill {container}`"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_not_running_error(&stderr) {
        return Ok(());
    }

    anyhow::bail!("`docker kill {container}` failed: {}", stderr.trim())
}

pub(crate) async fn running_container_name_by_id(
    container_id: &str,
) -> anyhow::Result<Option<String>> {
    let filter = format!("label={CONTAINER_ID_LABEL}={container_id}");
    let output = Command::new("docker")
        .args(["ps", "--filter", &filter, "--format", "{{.Names}}"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| {
            format!("failed to spawn `docker ps` for dcc container id {container_id}")
        })?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(command_failure("docker ps", code, &output.stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut names = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned);
    let first = names.next();
    if let (Some(first), Some(second)) = (&first, names.next()) {
        anyhow::bail!(
            "multiple running containers found for dcc container id `{container_id}`: `{first}`, `{second}`"
        );
    }
    Ok(first)
}

fn running_dcc_containers_args() -> [&'static str; 5] {
    [
        "ps",
        "--filter",
        "label=dcc.container_id",
        "--format",
        RUNNING_DCC_CONTAINERS_FORMAT,
    ]
}

/// Returns one point-in-time snapshot of all running containers carrying a dcc
/// container identity label. Callers decide how current, legacy, and unknown
/// role values affect their domain-specific status.
pub(crate) async fn running_dcc_containers() -> anyhow::Result<Vec<RunningDccContainer>> {
    let output = Command::new("docker")
        .args(running_dcc_containers_args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("failed to spawn `docker ps` for running dcc containers")?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(command_failure("docker ps", code, &output.stderr));
    }

    parse_running_dcc_containers(&output.stdout)
        .context("failed to parse running dcc containers from `docker ps`")
}

fn parse_running_dcc_containers(stdout: &[u8]) -> anyhow::Result<Vec<RunningDccContainer>> {
    let stdout = std::str::from_utf8(stdout).context("`docker ps` output was not valid UTF-8")?;
    stdout
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let mut fields = line.split('\t');
            let (Some(container_id), Some(role), Some(name)) =
                (fields.next(), fields.next(), fields.next())
            else {
                anyhow::bail!(
                    "record {} must contain exactly three tab-separated fields",
                    index + 1
                );
            };
            if fields.next().is_some() {
                anyhow::bail!(
                    "record {} must contain exactly three tab-separated fields",
                    index + 1
                );
            }

            if container_id.is_empty() {
                anyhow::bail!("record {} has an empty dcc container id", index + 1);
            }

            let role = (!role.is_empty()).then(|| role.to_owned());
            Ok(RunningDccContainer {
                container_id: container_id.to_owned(),
                role,
                name: name.to_owned(),
            })
        })
        .collect()
}

/// Reads the `devcontainer.metadata` label from a local Docker image.
/// Returns `None` when the image exists but the label is absent.
/// Returns `Err` when the image does not exist or the Docker daemon is unreachable.
pub(crate) async fn inspect_image_label(image: &str) -> anyhow::Result<Option<String>> {
    inspect_image_label_value(image, "devcontainer.metadata").await
}

pub(crate) async fn inspect_image_dcc_version(image: &str) -> anyhow::Result<Option<String>> {
    inspect_image_label_value(image, "dcc.version").await
}

pub(crate) async fn inspect_image_label_value(
    image: &str,
    label: &str,
) -> anyhow::Result<Option<String>> {
    let template = format!(r#"{{{{index .Config.Labels "{label}"}}}}"#);
    let output = Command::new("docker")
        .args(["image", "inspect", "--format", &template, image])
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
    if trimmed.is_empty() || trimmed == "<no value>" {
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

/// Probes the configured user's runtime `HOME` and `USER` by running a throwaway
/// container as that user. The runtime sets `HOME` from `/etc/passwd` (and we read
/// `USER` via `id`); neither is part of the static image `Config.Env`, which is why
/// `${containerEnv:HOME}`/`${containerEnv:USER}` need this probe. Returns a map with
/// `HOME` and/or `USER` (absent when the probe produced an empty value).
pub(crate) async fn probe_user_env(
    image: &str,
    user: &str,
) -> anyhow::Result<HashMap<String, String>> {
    let output = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-u",
            user,
            "--entrypoint",
            "sh",
            image,
            "-c",
            r#"printf '%s\n%s\n' "$HOME" "$(id -un)""#,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .with_context(|| format!("failed to spawn `docker run` to probe env in `{image}`"))?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(command_failure(
            &format!("docker run {image} (HOME/USER probe)"),
            code,
            &output.stderr,
        ));
    }

    Ok(parse_user_env(&String::from_utf8_lossy(&output.stdout)))
}

/// Parses the env-probe stdout: line 1 → `HOME`, line 2 → `USER`. Empty values are
/// skipped (left unset rather than mapped to an empty string).
fn parse_user_env(stdout: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut lines = stdout.lines();
    for key in ["HOME", "USER"] {
        if let Some(value) = lines.next() {
            let value = value.trim();
            if !value.is_empty() {
                map.insert(key.to_string(), value.to_string());
            }
        }
    }
    map
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
    fn running_dcc_containers_args_query_all_labeled_running_containers_once() {
        assert_eq!(
            running_dcc_containers_args(),
            [
                "ps",
                "--filter",
                "label=dcc.container_id",
                "--format",
                r#"{{.Label "dcc.container_id"}}\t{{.Label "dcc.container_role"}}\t{{.Names}}"#,
            ]
        );
    }

    #[test]
    fn parse_running_dcc_containers_accepts_current_and_legacy_records() {
        let records = parse_running_dcc_containers(
            b"dcc-one\truntime\tone\ndcc-two\tbuild-prep\ttwo\ndcc-old\t\told\n",
        )
        .unwrap();
        assert_eq!(
            records,
            vec![
                RunningDccContainer {
                    container_id: "dcc-one".to_string(),
                    role: Some("runtime".to_string()),
                    name: "one".to_string(),
                },
                RunningDccContainer {
                    container_id: "dcc-two".to_string(),
                    role: Some("build-prep".to_string()),
                    name: "two".to_string(),
                },
                RunningDccContainer {
                    container_id: "dcc-old".to_string(),
                    role: None,
                    name: "old".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_running_dcc_containers_accepts_empty_snapshot_and_crlf() {
        assert!(parse_running_dcc_containers(b"").unwrap().is_empty());
        assert_eq!(
            parse_running_dcc_containers(b"dcc-one\truntime\tone\r\n").unwrap(),
            vec![RunningDccContainer {
                container_id: "dcc-one".to_string(),
                role: Some("runtime".to_string()),
                name: "one".to_string(),
            }]
        );
    }

    #[test]
    fn parse_running_dcc_containers_rejects_malformed_records_without_partial_results() {
        for malformed in [
            b"dcc-one\truntime".as_slice(),
            b"dcc-one\truntime\tone\textra".as_slice(),
            b"\truntime\tone".as_slice(),
            b"dcc-one\truntime\tone\n\n".as_slice(),
        ] {
            let input = [b"dcc-valid\truntime\tvalid\n".as_slice(), malformed].concat();
            assert!(
                parse_running_dcc_containers(&input).is_err(),
                "expected malformed snapshot to fail: {input:?}"
            );
        }
    }

    #[test]
    fn parse_running_dcc_containers_rejects_non_utf8_output() {
        let err = parse_running_dcc_containers(b"dcc-one\truntime\t\xff").unwrap_err();
        assert!(err.to_string().contains("valid UTF-8"), "got: {err:#}");
    }

    #[test]
    fn build_args_for_stdin_context_are_deterministic() {
        let args = build_args(&DockerBuildOptions {
            tag: "dcc-img".to_string(),
            no_cache: true,
            pull: false,
            metadata_label: Some("[{}]".to_string()),
            seed_label: None,
            file: None,
            context_dir: None,
            build_args: vec![
                ("ZED".to_string(), "last".to_string()),
                ("ALPHA".to_string(), "first".to_string()),
            ],
            target: Some("dev".to_string()),
        });
        assert_eq!(
            args,
            vec![
                "build",
                "--no-cache",
                "--label",
                "devcontainer.metadata=[{}]",
                "--build-arg",
                "ALPHA=first",
                "--build-arg",
                "ZED=last",
                "--target",
                "dev",
                "--tag",
                "dcc-img",
                "-"
            ]
        );
    }

    #[test]
    fn build_args_include_seed_label_when_present() {
        let args = build_args(&DockerBuildOptions {
            tag: "dcc-img".to_string(),
            no_cache: false,
            pull: false,
            metadata_label: Some("[{}]".to_string()),
            seed_label: Some(r#"{"build_id":"x","entries":[]}"#.to_string()),
            file: None,
            context_dir: None,
            build_args: Vec::new(),
            target: None,
        });
        assert!(
            args.contains(&"--label".to_string()),
            "expected --label, got: {args:?}"
        );
        assert!(
            args.iter()
                .any(|a| a == "dcc.seed={\"build_id\":\"x\",\"entries\":[]}"),
            "expected dcc.seed label, got: {args:?}"
        );
        assert!(
            args.iter().any(|a| a == "devcontainer.metadata=[{}]"),
            "expected metadata label too, got: {args:?}"
        );
    }

    #[test]
    fn build_args_for_path_context_include_file_and_context() {
        let args = build_args(&DockerBuildOptions {
            tag: "dcc-img".to_string(),
            no_cache: false,
            pull: false,
            metadata_label: None,
            seed_label: None,
            file: Some(std::path::Path::new("/workspace/.devcontainer/Dockerfile").to_path_buf()),
            context_dir: Some(std::path::Path::new("/workspace").to_path_buf()),
            build_args: Vec::new(),
            target: None,
        });
        assert_eq!(
            args,
            vec![
                "build",
                "--tag",
                "dcc-img",
                "--file",
                "/workspace/.devcontainer/Dockerfile",
                "/workspace"
            ]
        );
    }

    #[test]
    fn build_args_include_pull_flag_when_set() {
        let args = build_args(&DockerBuildOptions {
            tag: "dcc-img".to_string(),
            no_cache: false,
            pull: true,
            metadata_label: None,
            seed_label: None,
            file: None,
            context_dir: None,
            build_args: Vec::new(),
            target: None,
        });
        assert!(
            args.contains(&"--pull".to_string()),
            "expected --pull, got: {args:?}"
        );
    }

    #[test]
    fn build_args_omit_pull_flag_when_unset() {
        let args = build_args(&DockerBuildOptions {
            tag: "dcc-img".to_string(),
            no_cache: false,
            pull: false,
            metadata_label: None,
            seed_label: None,
            file: None,
            context_dir: None,
            build_args: Vec::new(),
            target: None,
        });
        assert!(
            !args.contains(&"--pull".to_string()),
            "did not expect --pull, got: {args:?}"
        );
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

    #[test]
    fn parse_user_env_both_lines() {
        let m = parse_user_env("/home/dev\ndev\n");
        assert_eq!(m.get("HOME").map(String::as_str), Some("/home/dev"));
        assert_eq!(m.get("USER").map(String::as_str), Some("dev"));
    }

    #[test]
    fn parse_user_env_skips_empty_home() {
        let m = parse_user_env("\ndev\n");
        assert!(!m.contains_key("HOME"));
        assert_eq!(m.get("USER").map(String::as_str), Some("dev"));
    }

    #[test]
    fn parse_user_env_trims_and_handles_missing_second_line() {
        let m = parse_user_env("  /root  \n");
        assert_eq!(m.get("HOME").map(String::as_str), Some("/root"));
        assert!(!m.contains_key("USER"));
    }

    #[test]
    fn parse_user_env_empty() {
        assert!(parse_user_env("").is_empty());
    }
}

/// Bounded capture for runtime control. Neither snapshots nor command argv are logged.
pub(crate) async fn capture(args: &[String], limit: usize) -> anyhow::Result<std::process::Output> {
    use tokio::io::AsyncReadExt as _;
    let mut child = Command::new("docker")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("failed to start Docker runtime operation")?;
    let stdout = child.stdout.take().context("Docker stdout missing")?;
    let stderr = child.stderr.take().context("Docker stderr missing")?;
    async fn read<R: tokio::io::AsyncRead + Unpin>(
        reader: R,
        limit: usize,
    ) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        reader
            .take((limit + 1) as u64)
            .read_to_end(&mut bytes)
            .await?;
        anyhow::ensure!(
            bytes.len() <= limit,
            "Docker control output exceeds size limit"
        );
        Ok(bytes)
    }
    let result = tokio::try_join!(read(stdout, limit), read(stderr, 16384));
    match result {
        Ok((stdout, stderr)) => Ok(std::process::Output {
            status: child.wait().await?,
            stdout,
            stderr,
        }),
        Err(e) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(e)
        }
    }
}

pub(crate) fn captured_ok(output: &std::process::Output, operation: &str) -> anyhow::Result<()> {
    if output.status.success() {
        return Ok(());
    }
    Err(command_failure(
        operation,
        output.status.code().unwrap_or(-1),
        &output.stderr,
    ))
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ContainerInspect {
    pub(crate) id: String,
    pub(crate) image: String,
    pub(crate) config: ContainerConfig,
    pub(crate) state: ContainerState,
    pub(crate) network_settings: ContainerNetwork,
}
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ContainerConfig {
    pub(crate) labels: HashMap<String, String>,
}
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ContainerState {
    pub(crate) status: String,
    pub(crate) started_at: String,
}
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct ContainerNetwork {
    #[serde(default)]
    pub(crate) ports: HashMap<String, Option<Vec<PortBinding>>>,
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct PortBinding {
    #[serde(rename = "HostIp")]
    pub(crate) host_ip: String,
    #[serde(rename = "HostPort")]
    pub(crate) host_port: String,
}

pub(crate) async fn inspect_container(name: &str) -> anyhow::Result<Option<ContainerInspect>> {
    let output = capture(
        &["container".into(), "inspect".into(), name.into()],
        crate::runtime_snapshot::MAX_BYTES,
    )
    .await?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        if error.contains("No such container") || error.contains("No such object") {
            return Ok(None);
        }
        captured_ok(&output, "container inspect")?;
    }
    let mut records: Vec<ContainerInspect> =
        serde_json::from_slice(&output.stdout).context("invalid Docker container inspection")?;
    anyhow::ensure!(
        records.len() == 1,
        "expected exactly one inspected container"
    );
    Ok(records.pop())
}

pub(crate) async fn runtime_by_identity(
    identity: &str,
) -> anyhow::Result<Option<ContainerInspect>> {
    let output = capture(
        &[
            "ps".into(),
            "--filter".into(),
            format!("label={CONTAINER_ID_LABEL}={identity}"),
            "--format".into(),
            "{{.ID}}\t{{.Label \"dcc.container_role\"}}".into(),
        ],
        65536,
    )
    .await?;
    captured_ok(&output, "runtime lookup")?;
    let text = std::str::from_utf8(&output.stdout).context("invalid runtime lookup output")?;
    let mut names = Vec::new();
    for line in text.lines() {
        let (id, role) = line
            .split_once('\t')
            .context("invalid runtime lookup record")?;
        if role == CONTAINER_ROLE_BUILD_PREP {
            continue;
        }
        anyhow::ensure!(
            role.is_empty() || role == CONTAINER_ROLE_RUNTIME,
            "unrecognized dcc container role"
        );
        anyhow::ensure!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid container ID"
        );
        names.push(id);
    }
    anyhow::ensure!(
        names.len() <= 1,
        "multiple runtime containers match this profile; stop the extra instances explicitly"
    );
    match names.first() {
        None => Ok(None),
        Some(name) => inspect_container(name).await,
    }
}

pub(crate) async fn image_id(image: &str) -> anyhow::Result<String> {
    let output = capture(
        &[
            "image".into(),
            "inspect".into(),
            "--format".into(),
            "{{.Id}}".into(),
            image.into(),
        ],
        4096,
    )
    .await?;
    captured_ok(&output, "image identity inspection")?;
    let id = std::str::from_utf8(&output.stdout)?.trim();
    anyhow::ensure!(
        id.starts_with("sha256:")
            && id.len() == 71
            && id[7..].bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid immutable image ID"
    );
    Ok(id.to_owned())
}

pub(crate) async fn control(
    container: &str,
    verb: &str,
    argument: Option<&str>,
) -> anyhow::Result<std::process::Output> {
    let mut args = vec![
        "exec".into(),
        "-u".into(),
        "0".into(),
        "-w".into(),
        "/".into(),
        container.into(),
        format!("{}/dcc-ctl", crate::supervisor::DCC_SHARE),
        verb.into(),
    ];
    if let Some(value) = argument {
        args.push(value.into());
    }
    capture(&args, crate::runtime_snapshot::MAX_BYTES).await
}
