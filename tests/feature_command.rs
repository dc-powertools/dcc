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

#[test]
fn feature_jsonc_combined_edit_preserves_unrelated_configuration() {
    let fx = Fixture::new();
    let config = fx.write_config(
        "devcontainer.json",
        r#"{
            // JSONC comments and trailing commas are valid input.
            "name": "matrix fixture",
            "image": "rust:1",
            "mounts": ["type=volume,target=/data"],
            "features": {
                "ghcr.io/devcontainers/features/node:1": { "version": "22" },
            },
            "customizations": {
                "vscode": { "extensions": ["rust-lang.rust-analyzer"] },
            },
        }"#,
    );

    let output = fx
        .dcc(&[
            "--format",
            "json",
            "feature",
            "--remove",
            "ghcr.io/devcontainers/features/node:1",
            "--add",
            "ghcr.io/devcontainers/features/python:1",
        ])
        .output()
        .unwrap();
    assert_success(&output);

    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        summary["removed"],
        serde_json::json!(["ghcr.io/devcontainers/features/node:1"])
    );
    assert_eq!(
        summary["added"],
        serde_json::json!(["ghcr.io/devcontainers/features/python:1"])
    );

    let updated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config).unwrap()).unwrap();
    assert_eq!(updated["name"], "matrix fixture");
    assert_eq!(updated["image"], "rust:1");
    assert_eq!(
        updated["mounts"],
        serde_json::json!(["type=volume,target=/data"])
    );
    assert_eq!(
        updated["customizations"]["vscode"]["extensions"],
        serde_json::json!(["rust-lang.rust-analyzer"])
    );
    assert!(updated["features"]
        .get("ghcr.io/devcontainers/features/node:1")
        .is_none());
    assert_eq!(
        updated["features"]["ghcr.io/devcontainers/features/python:1"],
        serde_json::json!({})
    );
}

#[test]
fn feature_duplicate_additions_are_coalesced() {
    let fx = Fixture::new();
    let config = fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    let feature = "ghcr.io/devcontainers/features/node:1";

    let output = fx
        .dcc(&[
            "--format", "json", "feature", "--add", feature, "--add", feature,
        ])
        .output()
        .unwrap();
    assert_success(&output);

    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["added"], serde_json::json!([feature]));
    assert_eq!(summary["already_present"], serde_json::json!([]));
    let updated: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config).unwrap()).unwrap();
    assert_eq!(updated["features"].as_object().unwrap().len(), 1);
    assert_eq!(updated["features"][feature], serde_json::json!({}));
}

#[test]
fn feature_existing_add_and_missing_remove_report_noop_without_rewrite() {
    let fx = Fixture::new();
    let feature = "ghcr.io/devcontainers/features/node:1";
    let missing = "ghcr.io/devcontainers/features/python:1";
    let config = fx.write_config(
        "devcontainer.json",
        r#"{
            // A no-op must not rewrite this JSONC input.
            "image": "rust:1",
            "features": {
                "ghcr.io/devcontainers/features/node:1": { "version": "22" },
            },
        }"#,
    );
    let before = std::fs::read_to_string(&config).unwrap();

    let output = fx
        .dcc(&["feature", "--add", feature, "--remove", missing])
        .output()
        .unwrap();
    assert_success(&output);

    assert_eq!(std::fs::read_to_string(config).unwrap(), before);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("feature already present {feature}")));
    assert!(stdout.contains(&format!("feature not present {missing}")));
    assert!(stdout.contains("profile features unchanged"));
}
