mod common;
use common::*;

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn bootstrap_creates_default_profile_from_image() {
    let fx = Fixture::new();

    let output = fx
        .dcc(&["profile", "bootstrap", "--image", "debian:bookworm-slim"])
        .output()
        .unwrap();

    assert_success(&output);
    let path = fx.dir.path().join(".devcontainer/devcontainer.json");
    assert_eq!(
        read_json(&path),
        serde_json::json!({ "image": "debian:bookworm-slim" })
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("devcontainer"));
}

#[test]
fn bootstrap_uses_selected_profile_and_dockerfile() {
    let fx = Fixture::new();

    let output = fx
        .dcc(&[
            "profile",
            "bootstrap",
            "-p",
            "ci",
            "--dockerfile=Dockerfile.ci",
        ])
        .output()
        .unwrap();

    assert_success(&output);
    let path = fx.dir.path().join(".devcontainer/ci.json");
    assert_eq!(
        read_json(&path),
        serde_json::json!({
            "build": { "dockerfile": "Dockerfile.ci" }
        })
    );
    assert!(!fx
        .dir
        .path()
        .join(".devcontainer/devcontainer.json")
        .exists());
}

#[test]
fn bootstrap_extends_an_existing_valid_profile() {
    let fx = Fixture::new();
    fx.write_config("base.json", r#"{ "image": "rust:1" }"#);

    let output = fx
        .dcc(&["-p", "derived", "profile", "bootstrap", "--extends=base"])
        .output()
        .unwrap();

    assert_success(&output);
    let path = fx.dir.path().join(".devcontainer/derived.json");
    assert_eq!(
        read_json(&path),
        serde_json::json!({
            "customizations": { "dcc": { "extends": "./base.json" } }
        })
    );

    let validation = fx
        .dcc(&["--dry-run", "build", "-p", "derived"])
        .output()
        .unwrap();
    assert_success(&validation);
}

#[test]
fn bootstrap_refuses_to_overwrite_an_existing_profile() {
    let fx = Fixture::new();
    let path = fx.write_config("ci.json", "preserve me");

    let output = fx
        .dcc(&["profile", "bootstrap", "-p", "ci", "--image=rust:1"])
        .output()
        .unwrap();

    assert_failure(&output);
    assert_stderr_contains(&output, "already exists");
    assert_eq!(std::fs::read_to_string(path).unwrap(), "preserve me");
}

#[test]
fn bootstrap_requires_exactly_one_source() {
    let fx = Fixture::new();

    let missing = fx.dcc(&["profile", "bootstrap"]).output().unwrap();
    assert_failure(&missing);
    assert_stderr_contains(&missing, "--image");
    assert_stderr_contains(&missing, "--dockerfile");
    assert_stderr_contains(&missing, "--extends");

    let conflicting = fx
        .dcc(&["profile", "bootstrap", "--image=rust:1", "--extends=base"])
        .output()
        .unwrap();
    assert_failure(&conflicting);
    assert_stderr_contains(&conflicting, "cannot be used with");
}

#[test]
fn bootstrap_rejects_empty_source_values() {
    let fx = Fixture::new();

    for flag in ["--image=", "--dockerfile=", "--extends="] {
        let output = fx
            .dcc(&["profile", "bootstrap", flag, "-p", "new"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert_stderr_contains(&output, "non-empty");
        assert!(!fx.dir.path().join(".devcontainer/new.json").exists());
    }
}

#[test]
fn bootstrap_rejects_missing_or_invalid_parent_profiles_without_creating_target() {
    let fx = Fixture::new();

    let missing = fx
        .dcc(&["profile", "bootstrap", "-p", "child", "--extends=missing"])
        .output()
        .unwrap();
    assert_failure(&missing);
    assert_stderr_contains(&missing, "missing.json");
    assert!(!fx.dir.path().join(".devcontainer/child.json").exists());

    fx.write_config("invalid.json", r#"{ "features": {} }"#);
    let invalid = fx
        .dcc(&["profile", "bootstrap", "-p", "child", "--extends=invalid"])
        .output()
        .unwrap();
    assert_failure(&invalid);
    assert_stderr_contains(&invalid, "no `image` or `build`");
    assert!(!fx.dir.path().join(".devcontainer/child.json").exists());
}

#[test]
fn bootstrap_dry_run_validates_without_creating_profile() {
    let fx = Fixture::new();

    let output = fx
        .dcc(&[
            "--dry-run",
            "--format=json",
            "profile",
            "bootstrap",
            "-p",
            "ci",
            "--image=rust:1",
        ])
        .output()
        .unwrap();

    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "ok");
    assert_eq!(report["profile"], "ci");
    assert_eq!(report["docker_invoked"], false);
    assert!(!fx.dir.path().join(".devcontainer/ci.json").exists());
}

#[test]
fn bootstrap_rejects_path_and_nested_target_profiles() {
    let fx = Fixture::new();

    for profile in ["./new.json", "nested/new"] {
        let output = fx
            .dcc(&["profile", "bootstrap", "-p", profile, "--image=rust:1"])
            .output()
            .unwrap();
        assert_failure(&output);
        assert_stderr_contains(&output, "profile name");
    }
    assert!(!fx.dir.path().join("new.json").exists());
    assert!(!fx.dir.path().join(".devcontainer/nested").exists());
}
