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
