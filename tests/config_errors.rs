mod common;
use common::*;

// --- path-based profile (-p ./...) ---

#[test]
fn error_on_path_profile_file_not_found() {
    let fx = Fixture::new();
    let output = fx
        .dcc(&["build", "-p", "./nonexistent.json"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "nonexistent.json");
}

#[test]
fn path_profile_inside_workspace_loads_config() {
    let fx = Fixture::new();
    fx.write_config("../custom.json", r#"{ "image": "rust:1" }"#);
    let output = fx
        .dcc(&[
            "--dry-run",
            "--format",
            "json",
            "build",
            "-p",
            "./custom.json",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "ok");
    assert_eq!(report["profile"], "custom-json");
    assert_eq!(report["docker_invoked"], false);
    assert!(
        report["config"]
            .as_str()
            .is_some_and(|path| path.ends_with("/custom.json")),
        "dry-run did not load the requested config: {report}"
    );
}

#[test]
fn path_profile_container_id_consistent_across_commands() {
    let fx = Fixture::new();
    fx.write_config("claude.json", r#"{ "image": "rust:1" }"#);

    let profile = "./.devcontainer/claude.json";
    let id = fx
        .dcc(&["--format", "json", "id", "-p", profile])
        .output()
        .unwrap();
    let build = fx
        .dcc(&["--dry-run", "--format", "json", "build", "-p", profile])
        .output()
        .unwrap();
    let stop = fx
        .dcc(&["--dry-run", "--format", "json", "stop", "-p", profile])
        .output()
        .unwrap();
    assert_success(&id);
    assert_success(&build);
    assert_success(&stop);

    let id: serde_json::Value = serde_json::from_slice(&id.stdout).unwrap();
    let build: serde_json::Value = serde_json::from_slice(&build.stdout).unwrap();
    let stop: serde_json::Value = serde_json::from_slice(&stop.stdout).unwrap();
    assert_eq!(id["container_id"], build["container_id"]);
    assert_eq!(id["container_id"], stop["container_id"]);
    assert_eq!(id["profile"], "devcontainer-claude-json");
}

#[test]
fn error_on_missing_devcontainer_dir() {
    // Run from a temp dir with NO .devcontainer directory
    let dir = tempfile::tempdir().unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_dcc"))
        .arg("build")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "devcontainer");
}

#[test]
fn error_on_missing_profile_config() {
    let fx = Fixture::new();
    // .devcontainer/ exists but no profile file
    let output = fx
        .dcc(&["build", "--profile", "myprofile"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "myprofile");
}

#[test]
fn error_on_missing_default_profile_config() {
    let fx = Fixture::new();
    // .devcontainer/ exists but no devcontainer.json
    let output = fx.dcc(&["build"]).output().unwrap();
    assert_failure(&output);
    // Error should reference devcontainer.json or the profile name
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("devcontainer"),
        "expected 'devcontainer' in stderr, got: {stderr}"
    );
}

#[test]
fn error_on_circular_extends_two_files() {
    let fx = Fixture::new();
    fx.write_config("a.json", r#"{ "extends": "./b.json", "image": "rust:1" }"#);
    fx.write_config("b.json", r#"{ "extends": "./a.json", "image": "rust:1" }"#);
    let output = fx.dcc(&["build", "--profile", "a"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "circular");
}

#[test]
fn error_on_circular_extends_three_files() {
    let fx = Fixture::new();
    fx.write_config("a.json", r#"{ "extends": "./b.json", "image": "rust:1" }"#);
    fx.write_config("b.json", r#"{ "extends": "./c.json", "image": "rust:1" }"#);
    fx.write_config("c.json", r#"{ "extends": "./a.json", "image": "rust:1" }"#);
    let output = fx.dcc(&["build", "--profile", "a"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "circular");
}

#[test]
fn error_on_conflicting_legacy_and_nested_extends() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "extends": "./base.json",
            "customizations": { "dcc": { "extends": "./other.json" } }
        }"#,
    );
    let output = fx.dcc(&["build"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "top-level `extends`");
    assert_stderr_contains(&output, "customizations.dcc.extends");
}
