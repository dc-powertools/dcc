use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn metadata(dir: &Path, value: Value) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("devcontainer-feature.json"),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
}

fn validate(dir: &Path, upstream: bool) -> (Output, Value) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dcc"));
    // An executable-free PATH and a directory with no .devcontainer prove that
    // validation needs neither subprocesses nor workspace/profile discovery.
    cmd.current_dir(dir.parent().unwrap())
        .env("PATH", "")
        .args(["feature", "validate"])
        .arg(dir)
        .args(["--format", "json"]);
    if upstream {
        cmd.arg("--upstream-only");
    }
    let output = cmd.output().unwrap();
    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid report: {error}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output, report)
}

fn invalid(dir: &Path, value: Value, pointer: &str, message: &str) {
    metadata(dir, value);
    let (output, report) = validate(dir, false);
    assert_eq!(output.status.code(), Some(1), "{report}");
    assert_eq!(report["valid"], false);
    assert!(
        report["errors"].as_array().unwrap().iter().any(|error| {
            error["location"] == pointer
                && error["message"].as_str().unwrap().contains(message)
                && error["file"] == dir.join("devcontainer-feature.json").to_str().unwrap()
        }),
        "{report}"
    );
}

#[test]
fn single_feature_validates_offline_in_both_modes_without_install_script() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("hello");
    metadata(
        &dir,
        json!({"id":"hello", "version":"1.0.0", "mounts":[{"type":"volume","target":"/data"}],
        "dependsOn":{"ghcr.io/not-contacted/features/tool:1":{}}, "onCreateCommand":"exit 99"}),
    );
    for upstream in [false, true] {
        let (output, report) = validate(&dir, upstream);
        assert!(output.status.success(), "{report}");
        assert_eq!(report["valid"], true);
        assert_eq!(report["files"].as_array().unwrap().len(), 1);
        assert_eq!(
            report["mode"],
            if upstream { "upstream-only" } else { "dcc" }
        );
    }
}

#[test]
fn global_flags_work_before_validation_and_scripts_are_never_run() {
    let tmp = tempfile::tempdir().unwrap();
    metadata(tmp.path(), json!({"id":"hello","version":"1.0.0"}));
    fs::write(
        tmp.path().join("install.sh"),
        "#!/bin/sh\ntouch installed\nexit 99\n",
    )
    .unwrap();
    for args in [
        vec!["--format", "json", "feature", "validate", "."],
        vec!["feature", "--format", "json", "--dry-run", "validate", "."],
        vec![
            "feature",
            "validate",
            ".",
            "-p",
            "absent-profile",
            "--format",
            "json",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dcc"))
            .current_dir(tmp.path())
            .env("PATH", "")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["valid"], true);
    }
    assert!(!tmp.path().join("installed").exists());
    assert!(!tmp.path().join(".dcc").exists());
    assert!(!tmp.path().join(".devcontainer").exists());
}

#[test]
fn collection_validates_all_direct_features_in_sorted_order() {
    let tmp = tempfile::tempdir().unwrap();
    for id in ["z", "a"] {
        metadata(&tmp.path().join(id), json!({"id":id,"version":"1.0.0"}));
    }
    fs::write(tmp.path().join("README.md"), "collection").unwrap();
    fs::create_dir(tmp.path().join("non-feature")).unwrap();
    let (output, report) = validate(tmp.path(), false);
    assert!(output.status.success(), "{report}");
    assert_eq!(
        report["files"],
        json!([
            tmp.path().join("a/devcontainer-feature.json"),
            tmp.path().join("z/devcontainer-feature.json")
        ])
    );
    metadata(&tmp.path().join("a"), json!({"id":7,"version":"1.0.0"}));
    metadata(&tmp.path().join("z"), json!({"id":"z","version":false}));
    let (output, report) = validate(tmp.path(), false);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report["errors"].as_array().unwrap().len(), 2);
}

#[test]
fn extensions_work_only_in_dcc_mode() {
    let tmp = tempfile::tempdir().unwrap();
    for extension in [
        json!({"mounts":["type=bind,source=${localEnv:HOME},target=/host,readonly"]}),
        json!({"remoteEnv":{"HELLO":"world"}}),
        json!({"scripts":{"hello":"echo hello"}}),
    ] {
        let mut value = json!({"id":"hello","version":"1.0.0"});
        value
            .as_object_mut()
            .unwrap()
            .extend(extension.as_object().unwrap().clone());
        metadata(tmp.path(), value);
        let (output, report) = validate(tmp.path(), false);
        assert!(output.status.success(), "{report}");
        let (output, report) = validate(tmp.path(), true);
        assert_eq!(output.status.code(), Some(1), "{report}");
    }
}

#[test]
fn schema_rejects_missing_unknown_and_wrongly_typed_fields() {
    let tmp = tempfile::tempdir().unwrap();
    for (value, pointer, message) in [
        (json!({"id":"x"}), "", "version"),
        (json!({"id":3,"version":"1"}), "/id", "string"),
        (
            json!({"id":"x","version":"1","surprise":true}),
            "",
            "surprise",
        ),
        (
            json!({"id":"x","version":"1","privileged":"true"}),
            "/privileged",
            "boolean",
        ),
        (
            json!({"id":"x","version":"1","containerEnv":{"X":3}}),
            "/containerEnv/X",
            "string",
        ),
        (
            json!({"id":"x","version":"1","options":{"flag":{"type":"boolean","default":"yes"}}}),
            "/options/flag/default",
            "boolean",
        ),
        (
            json!({"id":"x","version":"1","mounts":[{"type":"bind","source":"/x","target":"/x","readonly":true}]}),
            "/mounts/0",
            "readonly",
        ),
        (
            json!({"id":"x","version":"1","customizations":{"dcc":{"commands":{"x":1}}}}),
            "/customizations/dcc/commands/x",
            "string",
        ),
        (
            json!({"id":"x","version":"1","customizations":{"dcc":{"registryCAs":{}}}}),
            "/customizations/dcc",
            "registryCAs",
        ),
    ] {
        invalid(tmp.path(), value, pointer, message);
    }
}

#[test]
fn parser_compatibility_checks_are_separate_from_upstream_schema() {
    let tmp = tempfile::tempdir().unwrap();
    for (addition, pointer, message) in [
        (
            json!({"mounts":[{"type":"bind","target":"/data"}]}),
            "/mounts/0",
            "source",
        ),
        (
            json!({"dependsOn":{"short:1":{}}}),
            "/dependsOn",
            "reference",
        ),
        (
            json!({"customizations":{"dcc":{"state":["relative"]}}}),
            "/customizations/dcc/state",
            "absolute",
        ),
    ] {
        let mut value = json!({"id":"hello","version":"1.0.0"});
        value
            .as_object_mut()
            .unwrap()
            .extend(addition.as_object().unwrap().clone());
        invalid(tmp.path(), value, pointer, message);
        let (output, report) = validate(tmp.path(), true);
        assert!(output.status.success(), "{report}");
    }
    invalid(
        tmp.path(),
        json!({"id":"x","version":"1","mounts":["type=volume"]}),
        "/mounts/0",
        "target",
    );
}

#[test]
fn supported_customizations_and_deferred_templates_validate() {
    let tmp = tempfile::tempdir().unwrap();
    metadata(
        tmp.path(),
        json!({"id":"x","version":"1","customizations":{
            "otherTool":{"anything":42}, "dcc":{"commands":{"hello":"echo hello"},
            "state":["${containerWorkspaceFolder}/target", {"path":"${containerEnv:HOME}/.history","type":"file"}]}
        }}),
    );
    for upstream in [false, true] {
        let (output, report) = validate(tmp.path(), upstream);
        assert!(output.status.success(), "{report}");
    }
}

#[test]
fn malformed_json_reports_file_line_and_column() {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("devcontainer-feature.json"), "{\n\"id\":").unwrap();
    let (output, report) = validate(tmp.path(), false);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report["errors"][0]["location"], "");
    assert!(report["errors"][0]["message"]
        .as_str()
        .unwrap()
        .contains("line 2 column"));
    assert_eq!(
        report["errors"][0]["file"],
        tmp.path()
            .join("devcontainer-feature.json")
            .to_str()
            .unwrap()
    );
}

#[test]
fn empty_missing_file_and_unreadable_metadata_inputs_fail() {
    let tmp = tempfile::tempdir().unwrap();
    let (output, report) = validate(tmp.path(), false);
    assert_eq!(output.status.code(), Some(1));
    assert!(report["errors"][0]["message"]
        .as_str()
        .unwrap()
        .contains("no Features"));
    let (output, _) = validate(&tmp.path().join("missing"), false);
    assert_eq!(output.status.code(), Some(1));
    fs::write(tmp.path().join("file"), "{}").unwrap();
    let (output, _) = validate(&tmp.path().join("file"), false);
    assert_eq!(output.status.code(), Some(1));
    // A directory in place of the metadata file fails even when tests run as root.
    fs::create_dir(tmp.path().join("devcontainer-feature.json")).unwrap();
    let (output, report) = validate(tmp.path(), false);
    assert_eq!(output.status.code(), Some(1));
    assert!(report["errors"][0]["message"]
        .as_str()
        .unwrap()
        .contains("cannot read metadata"));
}

#[test]
fn validation_cannot_be_combined_with_editing() {
    let tmp = tempfile::tempdir().unwrap();
    for args in [
        vec!["feature", "--add", "node", "validate", "."],
        vec!["feature", "validate", ".", "--remove", "node"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_dcc"))
            .current_dir(tmp.path())
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
    }
    assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
}

#[test]
fn text_errors_include_json_pointer_and_success_names_the_mode() {
    let tmp = tempfile::tempdir().unwrap();
    metadata(
        tmp.path(),
        json!({"id":"x","version":"1","mounts":["type=volume"]}),
    );
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dcc"));
    cmd.current_dir(tmp.path())
        .env("PATH", "")
        .args(["feature", "validate", "."]);
    let output = cmd.output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("devcontainer-feature.json#/mounts/0"),
        "{error}"
    );
    assert!(error.contains("target"), "{error}");
    metadata(tmp.path(), json!({"id":"x","version":"1"}));
    let output = cmd.arg("--upstream-only").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("(upstream-only)"));
}

#[cfg(unix)]
#[test]
fn broken_metadata_symlink_is_an_error_even_beside_a_valid_feature() {
    let tmp = tempfile::tempdir().unwrap();
    metadata(
        &tmp.path().join("valid"),
        json!({"id":"valid","version":"1"}),
    );
    let bad = tmp.path().join("bad");
    fs::create_dir(&bad).unwrap();
    std::os::unix::fs::symlink("missing.json", bad.join("devcontainer-feature.json")).unwrap();
    let (output, report) = validate(tmp.path(), false);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report["files"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["errors"][0]["file"],
        bad.join("devcontainer-feature.json").to_str().unwrap()
    );
}
