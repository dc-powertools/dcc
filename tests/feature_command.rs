mod common;
use common::*;

#[test]
fn feature_add_creates_features_object() {
    let fx = Fixture::new();
    let config = fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);

    let output = fx
        .dcc(&["feature", "--add", "ghcr.io/devcontainers/features/node:1"])
        .output()
        .unwrap();
    assert_success(&output);

    let updated = std::fs::read_to_string(config).unwrap();
    let json: serde_json::Value = serde_json::from_str(&updated).unwrap();
    assert_eq!(
        json["features"]["ghcr.io/devcontainers/features/node:1"],
        serde_json::json!({})
    );
}

#[test]
fn feature_short_flags_add_and_remove() {
    let fx = Fixture::new();
    let config = fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "features": {
                "ghcr.io/devcontainers/features/node:1": { "version": "22" }
            }
        }"#,
    );

    let output = fx
        .dcc(&[
            "feature",
            "-r",
            "ghcr.io/devcontainers/features/node:1",
            "-a",
            "ghcr.io/devcontainers/features/python:1",
        ])
        .output()
        .unwrap();
    assert_success(&output);

    let updated = std::fs::read_to_string(config).unwrap();
    let json: serde_json::Value = serde_json::from_str(&updated).unwrap();
    assert!(json["features"]
        .get("ghcr.io/devcontainers/features/node:1")
        .is_none());
    assert_eq!(
        json["features"]["ghcr.io/devcontainers/features/python:1"],
        serde_json::json!({})
    );
}

#[test]
fn feature_add_preserves_existing_options() {
    let fx = Fixture::new();
    let config = fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "features": {
                "ghcr.io/devcontainers/features/node:1": { "version": "22" }
            }
        }"#,
    );

    let output = fx
        .dcc(&["feature", "-a", "ghcr.io/devcontainers/features/node:1"])
        .output()
        .unwrap();
    assert_success(&output);

    let updated = std::fs::read_to_string(config).unwrap();
    let json: serde_json::Value = serde_json::from_str(&updated).unwrap();
    assert_eq!(
        json["features"]["ghcr.io/devcontainers/features/node:1"]["version"],
        "22"
    );
}

#[test]
fn feature_dry_run_does_not_write_config() {
    let fx = Fixture::new();
    let config = fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    let before = std::fs::read_to_string(&config).unwrap();

    let output = fx
        .dcc(&[
            "--dry-run",
            "feature",
            "--add",
            "ghcr.io/devcontainers/features/node:1",
        ])
        .output()
        .unwrap();
    assert_success(&output);

    assert_eq!(std::fs::read_to_string(config).unwrap(), before);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dry-run ok: command=feature"));
}

#[test]
fn feature_json_output_reports_changes() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);

    let output = fx
        .dcc(&[
            "--format",
            "json",
            "feature",
            "-a",
            "ghcr.io/devcontainers/features/node:1",
        ])
        .output()
        .unwrap();
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["added"][0], "ghcr.io/devcontainers/features/node:1");
    assert!(json["removed"].as_array().is_some_and(Vec::is_empty));
}

#[test]
fn feature_requires_an_operation() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);

    let output = fx.dcc(&["feature"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "at least one --add or --remove");
}
