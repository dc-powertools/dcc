mod common;
use common::*;

use std::process::{Command, Output};

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
        let _ = docker(&["rm", "-f", &id]);
        let _ = docker(&["rm", "-f", &format!("{id}-build-prep")]);
        let _ = docker(&["rmi", "-f", &id]);
        let _ = docker(&["rmi", "-f", &format!("{id}-base")]);
    }
}

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("failed to run docker")
}

fn assert_no_running_container(container_id: &str) {
    let output = docker(&[
        "ps",
        "--filter",
        &format!("label=dcc.container_id={container_id}"),
        "--format",
        "{{.Names}}",
    ]);
    assert_success(&output);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "",
        "expected no running container for {container_id}"
    );
}

fn assert_running_container(container_id: &str) {
    let output = docker(&[
        "ps",
        "--filter",
        &format!("label=dcc.container_id={container_id}"),
        "--format",
        "{{.Names}}",
    ]);
    assert_success(&output);
    assert!(
        !String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "expected running container for {container_id}"
    );
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
    assert_success(&fx.dcc(&[
        "exec",
        "/bin/sh",
        "-lc",
        "printf durable > /workspace/durable.txt",
    ]));
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
fn build_dry_run_reports_planned_seeding_without_docker() {
    let fx = DockerFixture::new();
    write_seeding_dockerfile(&fx);
    fx.write_config(&seeding_dir_config());

    let out = fx.dcc(&["build", "--dry-run", "--format", "json"]);
    assert_success(&out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("dry-run JSON parseable");
    assert_eq!(v["docker_invoked"], false);
    // The report lists seeding among the planned/skipped considerations.
    let joined = stdout.to_lowercase();
    assert!(
        joined.contains("seed"),
        "expected dry-run report to mention seeding, got: {stdout}"
    );
}

#[test]
#[ignore]
fn one_shot_container_leaves_no_host_side_bookkeeping() {
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));
    let container_id = fx.container_id();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.dcc(&["exec", "/bin/true"]));
    assert_no_running_container(&container_id);

    // The old <workspace>/.dcc/<profile>/runtime/ directory must not exist.
    let runtime_dir = fx
        .fx
        .dir
        .path()
        .join(".dcc")
        .join("devcontainer")
        .join("runtime");
    assert!(
        !runtime_dir.exists(),
        "host-side runtime bookkeeping should not exist after one-shot teardown"
    );
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
    // `dcc start` creates a durable container with no command registered, so the
    // supervisor's PRIMED flag is never set. A graceful `dcc stop` must still
    // tear it down (the stopping flag overrides the startup grace period).
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
fn fast_path_root_profile_unaffected_by_uid_remap() {
    // A root containerUser fast-path profile must build and run with no remap
    // step; root is skipped by definition.
    let fx = DockerFixture::new();
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root"
        }}"#
    ));

    let build = fx.dcc(&["build"]);
    assert_success(&build);
    // No remap ARGs leaked into the build (fast path pulls+tags, no Dockerfile).
    let stderr = String::from_utf8_lossy(&build.stderr);
    assert!(
        !stderr.contains("updateRemoteUserUID remap user"),
        "root fast path must not plan a remap, got stderr: {stderr}"
    );
    assert_success(&fx.dcc(&["exec", "/bin/sh", "-lc", "test \"$(id -u)\" -eq 0"]));
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
