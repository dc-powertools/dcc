mod common;
use common::*;

use std::{
    io::{Read as _, Write as _},
    net::{Shutdown, TcpListener, TcpStream},
    path::Path,
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

const IMAGE: &str = "debian:bookworm-slim";

struct DockerFixture {
    fx: Fixture,
}

impl DockerFixture {
    fn new() -> Self {
        Self { fx: Fixture::new() }
    }

    fn write_config(&self, content: &str) {
        self.fx.write_config("devcontainer.json", content);
    }

    fn write_file(&self, path: &str, content: &str) {
        let path = self.fx.dir.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("failed to create parent directory");
        }
        std::fs::write(path, content).expect("failed to write fixture file");
    }

    fn create_dir(&self, path: &str) {
        std::fs::create_dir_all(self.fx.dir.path().join(path))
            .expect("failed to create fixture directory");
    }

    fn read_file(&self, path: &str) -> String {
        std::fs::read_to_string(self.fx.dir.path().join(path)).expect("failed to read fixture file")
    }

    fn dcc(&self, args: &[&str]) -> Output {
        self.fx.dcc(args).output().expect("failed to run dcc")
    }

    fn dcc_with_env(&self, args: &[&str], key: &str, value: &str) -> Output {
        self.fx
            .dcc(args)
            .env(key, value)
            .output()
            .expect("failed to run dcc")
    }

    fn container_id(&self) -> String {
        let output = self.dcc(&["id"]);
        assert_success(&output);
        String::from_utf8(output.stdout)
            .expect("container id should be utf8")
            .trim()
            .to_string()
    }
}

impl Drop for DockerFixture {
    fn drop(&mut self) {
        let Ok(output) = self.fx.dcc(&["id"]).output() else {
            return;
        };
        if !output.status.success() {
            return;
        }
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if id.is_empty() {
            return;
        }
        // Cleanup must remain harmless for Docker-free tests and hosts. Live tests
        // still use `docker()` below so a missing executable fails their assertions.
        let _ = Command::new("docker").args(["rm", "-f", &id]).output();
        let _ = Command::new("docker")
            .args(["rm", "-f", &format!("{id}-build-prep")])
            .output();
        let _ = Command::new("docker").args(["rmi", "-f", &id]).output();
        let _ = Command::new("docker")
            .args(["rmi", "-f", &format!("{id}-base")])
            .output();
    }
}

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("failed to run docker")
}

fn running_container_instance_ids(container_id: &str) -> Vec<String> {
    let output = docker(&[
        "ps",
        "--filter",
        &format!("label=dcc.container_id={container_id}"),
        "--format",
        "{{.ID}}",
    ]);
    assert_success(&output);
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn running_container_instance_id(container_id: &str) -> String {
    let ids = running_container_instance_ids(container_id);
    assert_eq!(
        ids.len(),
        1,
        "expected exactly one running container for {container_id}, got {ids:?}"
    );
    ids[0].clone()
}

fn assert_no_running_container(container_id: &str) {
    assert_eq!(
        running_container_instance_ids(container_id),
        Vec::<String>::new(),
        "expected no running container for {container_id}"
    );
}

fn assert_running_container(container_id: &str) {
    let _ = running_container_instance_id(container_id);
}

fn assert_container_removed(instance_id: &str) {
    let output = docker(&["inspect", instance_id]);
    assert_failure(&output);
}

struct ChildGuard(Child);

impl ChildGuard {
    fn wait_for_file(&mut self, path: &Path, description: &str) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if path.exists() {
                return;
            }
            if let Some(status) = self.0.try_wait().expect("failed to poll dcc process") {
                panic!("dcc exited with {status} before {description}");
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {description}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn wait_for_success(&mut self, description: &str) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = self.0.try_wait().expect("failed to poll dcc process") {
                assert!(status.success(), "{description} failed with {status}");
                return;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {description}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore]
fn forwarded_port_reaches_container_loopback_service() {
    let reserved = TcpListener::bind("127.0.0.1:0").expect("failed to reserve host port");
    let port = reserved.local_addr().unwrap().port();
    drop(reserved);

    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "forwardPorts": [{port}]
        }}"#
    ));
    assert_success(&fx.dcc(&["build"]));

    let service = format!(
        "rm -f /tmp/dcc-forward-fifo; mkfifo /tmp/dcc-forward-fifo; \
         (printf container-response > /tmp/dcc-forward-fifo) & \
         nc -l 127.0.0.1 {port} < /tmp/dcc-forward-fifo > /workspace/forwarded-request.txt"
    );
    let mut command = fx.fx.dcc(&["exec", "/bin/sh", "-lc", &service]);
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to start dcc forwarding smoke");

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut response = Vec::new();
    while Instant::now() < deadline {
        let address = format!("127.0.0.1:{port}").parse().unwrap();
        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(250)) {
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            if stream.write_all(b"host-request").is_ok()
                && stream.shutdown(Shutdown::Write).is_ok()
                && stream.read_to_end(&mut response).is_ok()
                && response == b"container-response"
            {
                break;
            }
            response.clear();
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        response, b"container-response",
        "forwarded response mismatch"
    );

    let status = loop {
        if let Some(status) = child.try_wait().expect("failed to poll dcc process") {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("dcc forwarding smoke did not exit before its deadline");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(status.success(), "dcc forwarding smoke failed: {stderr}");
    assert_eq!(fx.read_file("forwarded-request.txt"), "host-request");
}

#[test]
#[ignore]
fn run_named_command_writes_workspace_marker() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "mark": "printf named > /workspace/named-command.txt"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "mark"]));

    assert_eq!(fx.read_file("named-command.txt"), "named");
}

#[test]
#[ignore]
fn lifecycle_hooks_run_in_expected_phases() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "initializeCommand": "printf 'initialize\n' >> hooks.log",
            "onCreateCommand": "printf 'onCreate\n' >> /workspace/hooks.log",
            "updateContentCommand": "printf 'updateContent\n' >> /workspace/hooks.log",
            "postCreateCommand": "printf 'postCreate\n' >> /workspace/hooks.log",
            "postStartCommand": "printf 'postStart\n' >> /workspace/hooks.log",
            "postAttachCommand": "printf 'postAttach\n' >> /workspace/hooks.log"
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    assert_eq!(
        fx.read_file("hooks.log"),
        "onCreate\nupdateContent\npostCreate\n"
    );

    assert_success(&fx.dcc(&["exec", "/bin/true"]));
    assert_eq!(
        fx.read_file("hooks.log"),
        "onCreate\nupdateContent\npostCreate\npostStart\n"
    );

    assert_success(&fx.dcc(&["attach", "/bin/true"]));
    assert_eq!(
        fx.read_file("hooks.log"),
        "onCreate\nupdateContent\npostCreate\npostStart\npostStart\npostAttach\n"
    );
}

#[test]
#[ignore]
fn state_directory_and_file_persist_across_one_shot_containers() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "customizations": {{
                "dcc": {{
                    "state": [
                        "/persist-dir",
                        {{ "path": "/persist-file", "type": "file" }},
                        {{ "path": "/persist-after-refresh", "type": "file" }}
                    ],
                    "commands": {{
                        "write": "printf dir > /persist-dir/value && printf file > /persist-file",
                        "read": "cat /persist-dir/value > /workspace/state-dir.txt && cat /persist-file > /workspace/state-file.txt",
                        "write-refresh": "printf refresh > /persist-after-refresh",
                        "read-refresh": "cat /persist-after-refresh > /workspace/state-refresh.txt"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "write"]));
    assert_success(&fx.dcc(&["run", "read"]));
    assert_eq!(fx.read_file("state-dir.txt"), "dir");
    assert_eq!(fx.read_file("state-file.txt"), "file");

    assert_success(&fx.dcc(&["run", "write-refresh"]));
    assert_success(&fx.dcc(&["build", "--refresh-only"]));
    assert_success(&fx.dcc(&["run", "read-refresh"]));
    assert_eq!(fx.read_file("state-refresh.txt"), "refresh");
}

#[test]
#[ignore]
fn durable_and_one_shot_container_modes_behave_differently() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "keep": "printf keep > /workspace/keep.txt"
                    }}
                }}
            }}
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["exec", "/bin/true"]));
    assert_no_running_container(&container_id);

    assert_success(&fx.dcc(&["start"]));
    assert_running_container(&container_id);
    let durable_instance = running_container_instance_id(&container_id);
    assert_success(&fx.dcc(&[
        "exec",
        "/bin/sh",
        "-lc",
        "printf durable > /workspace/durable.txt",
    ]));
    assert_eq!(
        running_container_instance_id(&container_id),
        durable_instance,
        "dcc exec should reuse the container started by dcc start"
    );
    assert_eq!(fx.read_file("durable.txt"), "durable");

    assert_success(&fx.dcc(&["stop"]));
    assert_no_running_container(&container_id);

    assert_success(&fx.dcc(&["run", "--keep", "keep"]));
    assert_running_container(&container_id);
    assert_eq!(fx.read_file("keep.txt"), "keep");
}

#[test]
#[ignore]
fn workspace_folder_sets_runtime_workdir() {
    let fx = DockerFixture::new();
    fx.create_dir("subdir");
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "workspaceFolder": "${{containerWorkspaceFolder}}/subdir",
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "pwd": "pwd > /workspace/pwd.txt"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "pwd"]));

    assert_eq!(fx.read_file("pwd.txt"), "/workspace/subdir\n");
}

#[test]
#[ignore]
fn runtime_environment_substitution_is_applied() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "containerEnv": {{
                "BAKED": "baked"
            }},
            "remoteEnv": {{
                "RUNTIME": "${{containerEnv:BAKED}}-runtime",
                "HOSTED": "${{localEnv:DCC_SMOKE_HOSTED:missing}}"
            }},
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "envcheck": "printf \"$BAKED|$RUNTIME|$HOSTED\" > /workspace/env.txt"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc_with_env(&["run", "envcheck"], "DCC_SMOKE_HOSTED", "hosted"));

    assert_eq!(fx.read_file("env.txt"), "baked|baked-runtime|hosted");
}

#[test]
#[ignore]
fn local_feature_commands_and_state_are_available_at_runtime() {
    let fx = DockerFixture::new();
    fx.write_file(
        ".devcontainer/smoke-feature/devcontainer-feature.json",
        r#"{
            "id": "smokefeat",
            "version": "1.0.0",
            "name": "Smoke Feature",
            "customizations": {
                "dcc": {
                    "state": ["/feature-state"],
                    "commands": {
                        "mark": "mkdir -p /feature-state && printf feature > /feature-state/value && cp /feature-state/value /workspace/feature-command.txt"
                    }
                }
            }
        }"#,
    );
    fx.write_file(
        ".devcontainer/smoke-feature/install.sh",
        "#!/bin/sh\nset -eu\n",
    );
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "features": {{
                "./smoke-feature": {{}}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "smokefeat:mark"]));
    assert_eq!(fx.read_file("feature-command.txt"), "feature");

    assert_success(&fx.dcc(&[
        "exec",
        "/bin/sh",
        "-lc",
        "cat /feature-state/value > /workspace/feature-state-copy.txt",
    ]));
    assert_eq!(fx.read_file("feature-state-copy.txt"), "feature");
}

// ── T-0022: state seeding from the image ──────────────────────────────────────

/// A config whose Dockerfile installs content at a declared directory state path,
/// so the bind mount would mask it without seeding.
fn seeding_dir_config() -> String {
    // Use a build source so the image actually contains /seeded-dir/value.
    r#"{
            "build": { "dockerfile": "Dockerfile" },
            "containerUser": "root",
            "customizations": {
                "dcc": {
                    "state": ["/seeded-dir"],
                    "commands": {
                        "read": "cat /seeded-dir/value > /workspace/seeded.txt"
                    }
                }
            }
        }"#
    .to_string()
}

fn write_seeding_dockerfile(fx: &DockerFixture) {
    fx.write_file(
        ".devcontainer/Dockerfile",
        &format!(
            r#"FROM {IMAGE}
RUN mkdir -p /seeded-dir && printf 'from-image' > /seeded-dir/value
RUN mkdir -p /seeded-file-dir && printf 'file-from-image' > /seeded-file-dir/.npmrc
"#,
        ),
    );
}

#[test]
#[ignore]
fn directory_state_seeded_from_image_on_build() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    fx.write_config(&seeding_dir_config());

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "read"]));
    assert_eq!(fx.read_file("seeded.txt"), "from-image");
}

#[test]
#[ignore]
fn file_state_seeded_from_image_on_build() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    fx.write_config(
        r#"{
            "build": { "dockerfile": "Dockerfile" },
            "containerUser": "root",
            "customizations": {
                "dcc": {
                    "state": [{ "path": "/seeded-file-dir/.npmrc", "type": "file" }],
                    "commands": {
                        "read": "cat /seeded-file-dir/.npmrc > /workspace/seeded-file.txt"
                    }
                }
            }
        }"#,
    );

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "read"]));
    assert_eq!(fx.read_file("seeded-file.txt"), "file-from-image");
}

#[test]
#[ignore]
fn build_prep_hook_observes_seeded_content() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    fx.write_config(
        r#"{
            "build": { "dockerfile": "Dockerfile" },
            "containerUser": "root",
            "onCreateCommand": "cat /seeded-dir/value > /workspace/hook-seed.txt",
            "customizations": {
                "dcc": {
                    "state": ["/seeded-dir"]
                }
            }
        }"#,
    );

    assert_success(&fx.dcc(&["build"]));
    assert_eq!(fx.read_file("hook-seed.txt"), "from-image");
}

#[test]
#[ignore]
fn wiped_dcc_rehydrates_from_image_without_rebuild() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    fx.write_config(&seeding_dir_config());

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "read"]));
    assert_eq!(fx.read_file("seeded.txt"), "from-image");

    // Wipe the profile cache (simulating a cloned repo with no .dcc). Seeded
    // state may contain root-owned directories (the hydration container runs as
    // root and tar preserves uid/gid), so wipe via a root container rather than
    // std::fs::remove_dir_all, which would get EACCES on root-owned dirs. Use
    // `find -mindepth 1 -delete` to remove the contents without removing the
    // bind-mount point itself (rm -rf /wipe fails with "Device or resource busy").
    let cache = fx.fx.dir.path().join(".dcc");
    let wipe = std::process::Command::new("docker")
        .args([
            "run",
            "--rm",
            "-u",
            "root",
            "-v",
            &format!("{}:/wipe", cache.display()),
            "debian:bookworm-slim",
            "find",
            "/wipe",
            "-mindepth",
            "1",
            "-delete",
        ])
        .output()
        .expect("failed to run docker wipe");
    assert_success(&wipe);

    assert_success(&fx.dcc(&["run", "read"]));
    assert_eq!(fx.read_file("seeded.txt"), "from-image");
}

#[test]
#[ignore]
fn modified_state_is_not_clobbered_on_rebuild() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    fx.write_config(&seeding_dir_config());

    assert_success(&fx.dcc(&["build"]));
    // Modify the seeded state from inside the container.
    assert_success(&fx.dcc(&[
        "exec",
        "/bin/sh",
        "-lc",
        "printf 'user-modified' > /seeded-dir/value",
    ]));

    let rebuild = fx.dcc(&["build"]);
    assert_success(&rebuild);
    // A warning must name the path and the digest/build-id divergence.
    let stderr = String::from_utf8_lossy(&rebuild.stderr);
    assert!(
        stderr.contains("/seeded-dir") && stderr.contains("reseed-state"),
        "expected no-clobber warning naming /seeded-dir and --reseed-state, got stderr: {stderr}"
    );

    // The user's modification survives the rebuild.
    assert_success(&fx.dcc(&["run", "read"]));
    assert_eq!(fx.read_file("seeded.txt"), "user-modified");
}

#[test]
#[ignore]
fn reseed_state_flag_clobbers_modified_state() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    fx.write_config(&seeding_dir_config());

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&[
        "exec",
        "/bin/sh",
        "-lc",
        "printf 'user-modified' > /seeded-dir/value",
    ]));

    assert_success(&fx.dcc(&["build", "--reseed-state"]));
    assert_success(&fx.dcc(&["run", "read"]));
    assert_eq!(fx.read_file("seeded.txt"), "from-image");
}

#[test]
#[ignore]
fn seeded_directory_is_writable_by_container_user() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    // Non-root user to confirm the seeded directory is writable by the dev
    // user in its standard bind-mounted location. updateRemoteUserUID (on by
    // default) remaps dev's uid/gid to the host's at build time, so the
    // workspace bind mount is writable regardless of the host runner's uid.
    fx.write_config(
        r#"{
            "build": { "dockerfile": "Dockerfile" },
            "containerUser": "dev",
            "customizations": {
                "dcc": {
                    "state": ["/seeded-dir"],
                    "commands": {
                        "write": "printf writable > /seeded-dir/after && cat /seeded-dir/after > /workspace/writable.txt"
                    }
                }
            }
        }"#,
    );
    // Ensure the dev user exists in the image.
    fx.write_file(
        ".devcontainer/Dockerfile",
        &format!(
            r#"FROM {IMAGE}
RUN id dev >/dev/null 2>&1 || useradd -m -s /bin/sh dev
RUN mkdir -p /seeded-dir && chown dev:dev /seeded-dir && printf 'from-image' > /seeded-dir/value
"#,
        ),
    );

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "write"]));
    // The command writes /workspace/writable.txt; read it back from the host
    // workspace bind-mount source.
    assert_eq!(fx.read_file("writable.txt"), "writable");
}

#[test]
#[ignore]
fn stop_now_force_terminates_durable_container() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["start"]));
    assert_running_container(&container_id);

    assert_success(&fx.dcc(&["stop", "--now"]));
    assert_no_running_container(&container_id);
}

#[test]
#[ignore]
fn stop_graceful_drains_durable_started_with_no_command() {
    // `dcc start` creates a durable container with no command registered. A
    // graceful `dcc stop` must still tear it down (durable containers never
    // reap and always honor the stopping flag).
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["start"]));
    assert_running_container(&container_id);

    assert_success(&fx.dcc(&["stop"]));
    assert_no_running_container(&container_id);
}

#[test]
#[ignore]
fn stop_kill_force_removes_wedged_container() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["start"]));
    assert_running_container(&container_id);

    assert_success(&fx.dcc(&["stop", "--kill"]));
    assert_no_running_container(&container_id);
}

#[test]
#[ignore]
fn stop_is_idempotent_when_no_container_running() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    // No container is running; all stop variants should succeed.
    assert_success(&fx.dcc(&["stop"]));
    assert_success(&fx.dcc(&["stop", "--now"]));
    assert_success(&fx.dcc(&["stop", "--kill"]));
}

#[test]
#[ignore]
fn command_exit_code_propagates_through_dcc_exec_wrapper() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));

    // A failing command should propagate its exit code through the dcc-exec wrapper.
    let output = fx.dcc(&["exec", "/bin/sh", "-lc", "exit 42"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(42));
}

#[test]
#[ignore]
fn non_root_user_writes_workspace_cache_and_state() {
    // updateRemoteUserUID (default true) remaps the dev user's uid/gid to the
    // host's at build time, so bind mounts are writable regardless of the host
    // runner's uid (e.g. GitHub Actions uid 1001 / gid 999).
    let fx = DockerFixture::new();
    fx.write_config(
        r#"{
            "build": { "dockerfile": "Dockerfile" },
            "containerUser": "dev",
            "customizations": {
                "dcc": {
                    "state": ["/seeded-dir"],
                    "commands": {
                        "wscratch": "printf ws > /workspace/scratch && printf cache > /cache/scratch && printf state > /seeded-dir/scratch"
                    }
                }
            }
        }"#,
    );
    fx.write_file(
        ".devcontainer/Dockerfile",
        &format!(
            r#"FROM {IMAGE}
RUN id dev >/dev/null 2>&1 || useradd -m -s /bin/sh dev
RUN mkdir -p /seeded-dir && chown dev:dev /seeded-dir
"#,
        ),
    );

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["run", "wscratch"]));
    assert_eq!(fx.read_file("scratch"), "ws");
    assert_eq!(
        std::fs::read_to_string(
            fx.fx
                .dir
                .path()
                .join(".dcc")
                .join("devcontainer")
                .join("scratch")
        )
        .unwrap_or_else(|_| panic!("expected cache scratch file")),
        "cache"
    );
    assert_eq!(
        std::fs::read_to_string(
            fx.fx
                .dir
                .path()
                .join(".dcc")
                .join("devcontainer")
                .join("state")
                .join("seeded-dir")
                .join("scratch")
        )
        .unwrap_or_else(|_| panic!("expected state scratch file")),
        "state"
    );
}

#[test]
#[ignore]
fn root_image_profile_builds_with_version_stamp_and_no_remap() {
    // A root containerUser image-only profile now goes through the dcc build
    // stage (the fast path was removed): it builds, gains a dcc.version label,
    // and plans no uid remap because root is skipped by definition.
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));

    let build = fx.dcc(&["build"]);
    assert_success(&build);
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        !stderr.contains("updateRemoteUserUID remap user"),
        "root profile must not plan a remap, got stderr: {stderr}"
    );
    assert_success(&fx.dcc(&["exec", "/bin/sh", "-lc", "test \"$(id -u)\" -eq 0"]));
    // The image carries a dcc.version label (stamped by build_dcc_stage).
    let image = fx.container_id();
    let out = docker(&[
        "image",
        "inspect",
        &image,
        "--format",
        r#"{{index .Config.Labels "dcc.version"}}"#,
    ]);
    let label = String::from_utf8_lossy(&out.stdout);
    assert!(
        !label.trim().is_empty(),
        "image should carry a dcc.version label, got: {label}"
    );
}

#[test]
#[ignore]
fn non_root_container_uid_matches_host_after_remap() {
    // After build with updateRemoteUserUID, the in-container uid of the dev
    // user matches the host process uid.
    let fx = DockerFixture::new();
    fx.write_config(
        r#"{
            "build": { "dockerfile": "Dockerfile" },
            "containerUser": "dev"
        }"#,
    );
    fx.write_file(
        ".devcontainer/Dockerfile",
        &format!(
            r#"FROM {IMAGE}
RUN id dev >/dev/null 2>&1 || useradd -m -s /bin/sh dev
"#,
        ),
    );

    assert_success(&fx.dcc(&["build"]));
    let host_uid = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "0".to_string());
    let out = fx.dcc(&["exec", "/bin/sh", "-lc", "id -u"]);
    assert_success(&out);
    let container_uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        container_uid, host_uid,
        "container uid `{container_uid}` should match host uid `{host_uid}` after remap"
    );
}

// ── T-0025: supervisor-owned startup handshake ────────────────────────────────

#[test]
#[ignore]
fn post_start_command_runs_in_supervisor_before_command() {
    // postStartCommand runs inside the supervisor (PID 1), and the foreground
    // command waits for readiness. A marker written by postStartCommand must
    // be visible to the command, proving ordering.
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "postStartCommand": "printf 'ready' > /workspace/startup-marker",
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "check": "cat /workspace/startup-marker"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    let out = fx.dcc(&["run", "check"]);
    assert_success(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ready"),
        "postStartCommand marker not visible to command; stdout: {stdout}"
    );
}

#[test]
#[ignore]
fn slow_post_start_command_does_not_race_command() {
    // A postStartCommand that sleeps longer than the old 60s grace would have
    // exceeded it. The command must still wait for readiness and succeed.
    // (Uses a 3s sleep — shorter than the old grace but long enough to prove
    // the command waits rather than racing.)
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "postStartCommand": "sleep 3 && printf 'done' > /workspace/slow-marker",
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "check": "cat /workspace/slow-marker"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    let out = fx.dcc(&["run", "check"]);
    assert_success(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("done"),
        "slow postStartCommand marker not visible; stdout: {stdout}"
    );
}

#[test]
#[ignore]
fn failing_post_start_command_surfaces_as_error() {
    // A failing postStartCommand must surface as a clear host-side error
    // (exit 252 mapped to a message), not a hang.
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "postStartCommand": "exit 42"
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    let out = fx.dcc(&["exec", "/bin/true"]);
    assert_failure(&out);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("startup failed") || stderr.contains("hook"),
        "failing postStartCommand should surface a clear error; stderr: {stderr}"
    );
}

#[test]
#[ignore]
fn start_then_immediate_stop_graceful_tears_down() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["start"]));
    assert_running_container(&container_id);
    // Immediate stop — no grace window dependency.
    assert_success(&fx.dcc(&["stop"]));
    assert_no_running_container(&container_id);
}

#[test]
#[ignore]
fn start_then_immediate_stop_now_tears_down() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["start"]));
    assert_running_container(&container_id);
    assert_success(&fx.dcc(&["stop", "--now"]));
    assert_no_running_container(&container_id);
}

#[test]
#[ignore]
fn start_then_immediate_stop_kill_tears_down() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["start"]));
    assert_running_container(&container_id);
    assert_success(&fx.dcc(&["stop", "--kill"]));
    assert_no_running_container(&container_id);
}

#[test]
#[ignore]
fn skip_lifecycle_skips_post_start_command() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "postStartCommand": "printf 'ran' > /workspace/skip-marker",
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "check": "test -f /workspace/skip-marker && echo exists || echo absent"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    let out = fx.dcc(&[
        "exec",
        "--skip-lifecycle",
        "/bin/sh",
        "-lc",
        "test -f /workspace/skip-marker && echo exists || echo absent",
    ]);
    assert_success(&out);
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        stdout, "absent",
        "postStartCommand should not have run under --skip-lifecycle; stdout: {stdout}"
    );
}

#[test]
#[ignore]
fn one_shot_container_drains_after_command_without_grace() {
    // A one-shot container must drain and exit after the command completes,
    // with no time-based grace. The container should be gone immediately
    // after dcc exec returns.
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    let ready = fx.fx.dir.path().join("one-shot-ready");
    let release = fx.fx.dir.path().join("one-shot-release");
    let mut command = fx.fx.dcc(&[
        "exec",
        "/bin/sh",
        "-lc",
        "printf ready > /workspace/one-shot-ready; while [ ! -f /workspace/one-shot-release ]; do sleep 0.05; done",
    ]);
    let mut process = ChildGuard(
        command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start one-shot dcc command"),
    );
    process.wait_for_file(&ready, "one-shot command registration");
    let original_instance = running_container_instance_id(&container_id);

    std::fs::write(release, b"release").unwrap();
    process.wait_for_success("one-shot dcc command");

    assert_container_removed(&original_instance);
    assert_no_running_container(&container_id);
}

#[test]
#[ignore]
fn durable_reuse_and_keep_promotion_work_with_handshake() {
    // A durable container (--keep) must support multiple commands, with the
    // readiness handshake working on reuse (fast path, no blocking).
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "postStartCommand": "count=$(cat /workspace/start-count 2>/dev/null || printf 0); printf '%s' $((count + 1)) > /workspace/start-count",
            "customizations": {{
                "dcc": {{
                    "commands": {{
                        "check": "cat /workspace/start-count"
                    }}
                }}
            }}
        }}"#
    ));

    assert_success(&fx.dcc(&["build"]));
    // First command with --keep: cold start, hooks run, command waits.
    let out = fx.dcc(&["run", "--keep", "check"]);
    assert_success(&out);
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1");
    let original_instance = running_container_instance_id(&fx.container_id());
    // Second command with --keep: reuses the durable container, fast-path readiness.
    let out2 = fx.dcc(&["run", "--keep", "check"]);
    assert_success(&out2);
    assert_eq!(String::from_utf8_lossy(&out2.stdout).trim(), "1");
    assert_eq!(
        running_container_instance_id(&fx.container_id()),
        original_instance,
        "second --keep command recreated the durable container"
    );
    // Clean up.
    assert_success(&fx.dcc(&["stop", "--now"]));
}
