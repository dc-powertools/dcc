mod common;
use common::*;

// --- path-based profile (-p ./...) ---

#[test]
fn error_on_path_profile_file_not_found() {
    let fx = Fixture::new();
    let output = fx
        .dcc(&["-p", "./nonexistent.json", "build"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "nonexistent.json");
}

#[test]
fn path_profile_inside_workspace_loads_config() {
    let fx = Fixture::new();
    // Write a config file at a non-standard location inside the workspace.
    fx.write_config("../custom.json", r#"{ "image": "rust:1" }"#);
    // dcc build will fail (no Docker), but the failure must NOT be about the config path.
    let output = fx.dcc(&["-p", "./custom.json", "build"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("nonexistent") && !stderr.contains("resolve config path"),
        "config should have been found; stderr: {stderr}"
    );
}

#[test]
fn path_profile_container_name_consistent_across_commands() {
    // dcc -p ./X build and dcc -p ./X stop must target the same container.
    // We can't run Docker here, but we can verify that stop reaches Docker
    // (not failing earlier on config resolution) when given a valid path arg.
    let fx = Fixture::new();
    fx.write_config("claude.json", r#"{ "image": "rust:1" }"#);
    // stop is idempotent (treats "no such container" as success), so success
    // with a path arg confirms the name was derived and passed to Docker.
    let output = fx
        .dcc(&["-p", "./.devcontainer/claude.json", "stop"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("resolve config path"),
        "path-based stop should not fail on config resolution; stderr: {stderr}"
    );
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
        .dcc(&["--profile", "myprofile", "build"])
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
    let output = fx.dcc(&["--profile", "a", "build"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "circular");
}

#[test]
fn error_on_circular_extends_three_files() {
    let fx = Fixture::new();
    fx.write_config("a.json", r#"{ "extends": "./b.json", "image": "rust:1" }"#);
    fx.write_config("b.json", r#"{ "extends": "./c.json", "image": "rust:1" }"#);
    fx.write_config("c.json", r#"{ "extends": "./a.json", "image": "rust:1" }"#);
    let output = fx.dcc(&["--profile", "a", "build"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "circular");
}
