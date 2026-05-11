mod common;
use common::*;

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
