mod common;
use common::*;

#[test]
fn strict_rejects_unknown_fields() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{ "image": "rust:1", "unknownField": "value" }"#,
    );
    let output = fx.dcc(&["--strict", "build"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "unknownField");
}

#[test]
fn strict_after_subcommand_rejects_unknown_fields() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{ "image": "rust:1", "unknownField": "value" }"#,
    );
    let output = fx.dcc(&["build", "--strict"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "unknownField");
}

#[test]
fn strict_exec_accepts_devcontainer_name_field() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{ "name": "example/project", "image": "rust:1" }"#,
    );
    let output = fx
        .dcc(&["--strict", "--dry-run", "exec", "echo", "OK"])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn strict_accepts_customizations_dcc_config() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "customizations": {
                "dcc": {
                    "commands": { "build": "cargo build" },
                    "state": ["/home/dev/.cache"]
                }
            }
        }"#,
    );
    let output = fx
        .dcc(&["--strict", "--dry-run", "build"])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn default_mode_warns_on_unknown_fields_but_does_not_fail_early() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{ "image": "rust:1", "unknownField": "value" }"#,
    );
    // Dry-run makes the intended non-strict success observable without Docker.
    // Set RUST_LOG=warn so tracing::warn! output appears in stderr.
    let output = fx
        .dcc(&["--dry-run", "build"])
        .env("RUST_LOG", "warn")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_success(&output);
    // The unknown field should have produced a warning (appears in stderr)
    assert!(
        stderr.contains("unknownField"),
        "expected warning about 'unknownField' in stderr, got: {stderr}"
    );
    // Should NOT have bailed due to strict-mode unknown-field error
    assert!(
        !stderr.to_lowercase().contains("unrecognized field"),
        "non-strict mode should not produce a fatal 'unrecognized field' error"
    );
}

#[test]
fn default_mode_warns_on_legacy_dcc_fields() {
    let fx = Fixture::new();
    fx.write_config("base.json", r#"{ "image": "rust:1" }"#);
    fx.write_config(
        "devcontainer.json",
        r#"{
            "extends": "base.json",
            "scripts": { "build": "cargo build" }
        }"#,
    );
    let output = fx
        .dcc(&["--dry-run", "build"])
        .env("RUST_LOG", "warn")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_success(&output);
    assert!(
        stderr.contains("top-level `extends` is deprecated"),
        "expected deprecation warning for legacy extends, got: {stderr}"
    );
    assert!(
        stderr.contains("top-level `scripts` is deprecated"),
        "expected deprecation warning for legacy scripts, got: {stderr}"
    );
    assert!(
        !stderr.to_lowercase().contains("unrecognized field"),
        "legacy fields should warn, not fail as unknown fields: {stderr}"
    );
}

#[test]
fn initialize_command_is_parse_only_and_warns() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "initializeCommand": "echo should-not-run"
        }"#,
    );
    let output = fx
        .dcc(&["--dry-run", "build"])
        .env("RUST_LOG", "warn")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_success(&output);
    assert!(
        stderr.contains("initializeCommand is parsed for devcontainer compatibility"),
        "expected unsupported initializeCommand warning, got: {stderr}"
    );
}

#[test]
fn strict_mode_warns_on_legacy_dcc_fields() {
    let fx = Fixture::new();
    fx.write_config("base.json", r#"{ "image": "rust:1" }"#);
    fx.write_config(
        "devcontainer.json",
        r#"{
            "extends": "base.json",
            "scripts": { "build": "cargo build" }
        }"#,
    );
    let output = fx
        .dcc(&["--strict", "build"])
        .arg("--dry-run")
        .env("RUST_LOG", "warn")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_success(&output);
    assert!(
        stderr.contains("top-level `extends` is deprecated"),
        "expected strict-mode deprecation warning for legacy extends, got: {stderr}"
    );
    assert!(
        stderr.contains("top-level `scripts` is deprecated"),
        "expected strict-mode deprecation warning for legacy scripts, got: {stderr}"
    );
    assert!(
        !stderr.to_lowercase().contains("unrecognized field"),
        "legacy fields should warn in strict mode, not fail as unknown fields: {stderr}"
    );
}

#[test]
fn dash_dash_not_rejected_by_arg_parser() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    // `dcc run --` should be accepted syntactically (may fail due to missing Docker)
    let output = fx.dcc(&["--dry-run", "run", "--"]).output().unwrap();
    assert_success(&output);
}

#[test]
fn positional_args_after_run_accepted() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "customizations": { "dcc": { "commands": { "true": "/bin/true" } } }
        }"#,
    );
    let output = fx.dcc(&["--dry-run", "run", "true"]).output().unwrap();
    assert_success(&output);
}

#[test]
fn profile_flag_before_and_after_subcommand_are_equivalent() {
    let fx = Fixture::new();
    // `dcc id` resolves the profile and prints the dcc container id; it needs
    // neither a Docker daemon nor a config file on disk for a named profile.
    let before = fx.dcc(&["-p", "base", "id"]).output().unwrap();
    let after = fx.dcc(&["id", "-p", "base"]).output().unwrap();
    assert_success(&before);
    assert_success(&after);
    assert_eq!(
        before.stdout,
        after.stdout,
        "`-p base` before and after the subcommand must produce identical output\n\
         before: {}\nafter: {}",
        String::from_utf8_lossy(&before.stdout),
        String::from_utf8_lossy(&after.stdout),
    );
    assert!(
        String::from_utf8_lossy(&before.stdout).contains("base"),
        "container id should reflect the `base` profile, got: {}",
        String::from_utf8_lossy(&before.stdout),
    );
}

#[test]
fn long_profile_flag_before_subcommand_accepted() {
    let fx = Fixture::new();
    let output = fx.dcc(&["--profile", "base", "id"]).output().unwrap();
    assert_success(&output);
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("base"),
        "container id should reflect the `base` profile, got: {}",
        String::from_utf8_lossy(&output.stdout),
    );
}

#[test]
fn id_ignores_devcontainer_name_field() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{ "name": "human-readable-name", "image": "rust:1" }"#,
    );
    let output = fx.dcc(&["id"]).output().unwrap();
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("dcc-"), "expected dcc id, got: {stdout}");
    assert!(
        !stdout.contains("human-readable-name"),
        "`dcc id` should print the stable dcc id, not devcontainer `name`"
    );
}

#[test]
fn profile_flag_before_subcommand_overrides_default() {
    let fx = Fixture::new();
    let with_profile = fx.dcc(&["-p", "base", "id"]).output().unwrap();
    let default = fx.dcc(&["id"]).output().unwrap();
    assert_success(&with_profile);
    assert_success(&default);
    assert_ne!(
        with_profile.stdout, default.stdout,
        "`-p base` before the subcommand should differ from the default profile"
    );
}

#[test]
fn strict_flag_after_subcommand_accepted() {
    let fx = Fixture::new();
    let output = fx.dcc(&["id", "--strict"]).output().unwrap();
    assert_success(&output);
}

#[test]
fn skip_lifecycle_flag_accepted_by_exec() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    // `--skip-lifecycle` must precede the trailing command; dry-run proves the
    // complete pre-Docker validation path accepts it.
    let output = fx
        .dcc(&["--dry-run", "exec", "--skip-lifecycle", "/bin/true"])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn debug_flag_is_global() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "customizations": { "dcc": { "commands": { "noop": "true" } } }
        }"#,
    );
    for args in [
        ["--debug", "--dry-run", "build"].as_slice(),
        ["--dry-run", "build", "--debug"].as_slice(),
        ["--dry-run", "exec", "--debug", "/bin/true"].as_slice(),
        ["--dry-run", "start", "--debug"].as_slice(),
        ["--dry-run", "attach", "--debug", "/bin/true"].as_slice(),
        ["--dry-run", "stop", "--debug"].as_slice(),
        ["id", "--debug"].as_slice(),
        ["--dry-run", "run", "--debug", "noop"].as_slice(),
    ] {
        let output = fx.dcc(args).output().unwrap();
        assert_success(&output);
    }
}

#[test]
fn debug_output_is_emitted_for_new_global_commands() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);

    let build = fx.dcc(&["--dry-run", "--debug", "build"]).output().unwrap();
    assert_success(&build);
    assert_stderr_contains(&build, "dcc debug: command `build`");

    let stop = fx.dcc(&["--dry-run", "--debug", "stop"]).output().unwrap();
    assert_success(&stop);
    assert_stderr_contains(&stop, "dcc debug: command `stop`");

    let id = fx.dcc(&["--debug", "id"]).output().unwrap();
    assert_success(&id);
    assert_stderr_contains(&id, "dcc debug: container id");
}

#[test]
fn allow_unsafe_runtime_flag_accepted_by_build_exec_and_run() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "customizations": { "dcc": { "commands": { "noop": "true" } } }
        }"#,
    );
    for args in [
        ["--dry-run", "build", "--allow-unsafe-runtime"].as_slice(),
        ["--dry-run", "exec", "--allow-unsafe-runtime", "/bin/true"].as_slice(),
        ["--dry-run", "run", "--allow-unsafe-runtime", "noop"].as_slice(),
    ] {
        let output = fx.dcc(args).output().unwrap();
        assert_success(&output);
    }
}

#[test]
fn start_and_attach_commands_are_accepted() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    for args in [
        ["--dry-run", "start"].as_slice(),
        ["--dry-run", "attach"].as_slice(),
        ["--dry-run", "attach", "/bin/sh"].as_slice(),
    ] {
        let output = fx.dcc(args).output().unwrap();
        assert_success(&output);
    }
}

#[test]
fn keep_flag_accepted_by_run_exec_and_attach() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "customizations": { "dcc": { "commands": { "noop": "true" } } }
        }"#,
    );
    for args in [
        ["--dry-run", "run", "--keep", "noop"].as_slice(),
        ["--dry-run", "run", "-k", "noop"].as_slice(),
        ["--dry-run", "exec", "--keep", "/bin/true"].as_slice(),
        ["--dry-run", "exec", "-k", "/bin/true"].as_slice(),
        ["--dry-run", "attach", "--keep"].as_slice(),
        ["--dry-run", "attach", "-k", "/bin/sh"].as_slice(),
    ] {
        let output = fx.dcc(args).output().unwrap();
        assert_success(&output);
    }
}

#[test]
fn refresh_only_flag_accepted_by_build() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    let output = fx
        .dcc(&["--dry-run", "build", "--refresh-only"])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn update_flag_rejected_by_build() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    let output = fx
        .dcc(&["--dry-run", "build", "--update"])
        .output()
        .unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "unexpected argument '--update'");
}

#[test]
fn stop_now_and_kill_flags_accepted() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    for args in [
        ["--dry-run", "stop", "--now"].as_slice(),
        ["--dry-run", "stop", "--kill"].as_slice(),
    ] {
        let output = fx.dcc(args).output().unwrap();
        assert_success(&output);
    }
}

#[test]
fn stop_dry_run_reports_action_for_each_variant() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);

    let graceful = fx
        .dcc(&["--dry-run", "--format", "json", "stop"])
        .output()
        .unwrap();
    assert_success(&graceful);
    let graceful_json = String::from_utf8_lossy(&graceful.stdout);
    assert!(
        graceful_json.contains("dcc-ctl stop (graceful drain)"),
        "graceful dry-run should report drain action: {graceful_json}"
    );

    let now = fx
        .dcc(&["--dry-run", "--format", "json", "stop", "--now"])
        .output()
        .unwrap();
    assert_success(&now);
    let now_json = String::from_utf8_lossy(&now.stdout);
    assert!(
        now_json.contains("dcc-ctl stop-now"),
        "--now dry-run should report stop-now action: {now_json}"
    );

    let kill = fx
        .dcc(&["--dry-run", "--format", "json", "stop", "--kill"])
        .output()
        .unwrap();
    assert_success(&kill);
    let kill_json = String::from_utf8_lossy(&kill.stdout);
    assert!(
        kill_json.contains("docker kill"),
        "--kill dry-run should report docker kill action: {kill_json}"
    );
}

#[test]
fn dry_run_format_json_outputs_stable_report() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    let output = fx
        .dcc(&["--dry-run", "--format", "json", "build", "--refresh-only"])
        .output()
        .unwrap();
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|e| panic!("invalid json: {e}\n{stdout}"));
    assert_eq!(json["status"], "ok");
    assert_eq!(json["command"], "build --refresh-only");
    assert_eq!(json["docker_invoked"], false);
    assert_eq!(json["profile"], "devcontainer");
    assert!(
        json["container_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("dcc-") && id.ends_with("--devcontainer")),
        "expected resolved container identity in dry-run report: {json}"
    );
    assert!(
        json["skipped"].as_array().is_some_and(|items| items
            .iter()
            .any(|item| item == "profile image existence check")),
        "expected skipped image-existence check in dry-run report: {json}"
    );
}

#[test]
fn build_dry_run_reports_planned_seeding_with_no_executables_on_path() {
    let fx = Fixture::new();
    fx.write_config(
        "Dockerfile",
        "FROM debian:bookworm-slim\nRUN mkdir -p /seeded-dir\n",
    );
    fx.write_config(
        "devcontainer.json",
        r#"{
            "build": { "dockerfile": "Dockerfile" },
            "containerUser": "root",
            "customizations": { "dcc": { "state": ["/seeded-dir"] } }
        }"#,
    );

    let output = fx
        .dcc(&["--dry-run", "--format", "json", "build"])
        // This directory contains neither Docker nor ordinary host utilities.
        .env("PATH", fx.dir.path())
        .output()
        .unwrap();
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["docker_invoked"], false);
    assert!(
        report["checks"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(report["skipped"].as_array().into_iter().flatten())
            .any(|entry| entry.as_str().is_some_and(|text| text.contains("seed"))),
        "expected dry-run seeding plan: {report}"
    );
}

#[test]
fn strict_accepts_official_build_source_field() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "build": {
                "context": "..",
                "dockerfile": "Dockerfile",
                "args": { "VERSION": "1" },
                "target": "dev"
            }
        }"#,
    );
    let output = fx
        .dcc(&["--strict", "--dry-run", "build"])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn strict_accepts_final_compatibility_fields() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "portsAttributes": {
                "3000": { "label": "web", "protocol": "http", "onAutoForward": "openBrowser" },
                "3001": { "onAutoForward": "openBrowserOnce" },
                "3002": { "onAutoForward": "openPreview" },
                "3003": { "onAutoForward": "silent" },
                "3004": { "onAutoForward": "ignore" }
            },
            "otherPortsAttributes": { "label": "other", "protocol": "https", "onAutoForward": "silent" },
            "runArgs": ["--add-host", "host.docker.internal:host-gateway"],
            "overrideCommand": false,
            "workspaceFolder": "${containerWorkspaceFolder}/service",
            "workspaceMount": "source=${localWorkspaceFolder},target=/workspace,type=bind"
        }"#,
    );
    let output = fx
        .dcc(&["--strict", "--dry-run", "build"])
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn build_rejects_devcontainer_unsafe_runtime_without_flag_before_docker() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "privileged": true,
            "capAdd": ["SYS_PTRACE"],
            "securityOpt": ["seccomp=unconfined"]
        }"#,
    );
    let output = fx.dcc(&["build"]).output().unwrap();
    assert_failure(&output);
    assert_stderr_contains(
        &output,
        "devcontainer config contains unsafe runtime setting",
    );
    assert_stderr_contains(&output, "--allow-unsafe-runtime");
    assert_stderr_contains(&output, "privileged");
    assert_stderr_contains(&output, "capAdd");
    assert_stderr_contains(&output, "securityOpt");
}

// Tests below require a live Docker daemon. They stay ignored for local/devcontainer
// cargo test runs; GitHub Actions runs them explicitly on an Ubuntu Docker host.
#[test]
#[ignore]
fn strict_accepts_valid_config() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    let output = fx.dcc(&["--strict", "build"]).output().unwrap();
    assert_success(&output);
}

#[test]
#[ignore]
fn exec_runs_direct_command() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    // Build first
    assert_success(&fx.dcc(&["build"]).output().unwrap());
    // Then execute an explicit command.
    assert_success(&fx.dcc(&["exec", "/bin/true"]).output().unwrap());
}

#[test]
#[ignore]
fn run_executes_named_command() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{
            "image": "rust:1",
            "customizations": {
                "dcc": {
                    "commands": {
                        "true": "/bin/true"
                    }
                }
            }
        }"#,
    );
    // Build first
    assert_success(&fx.dcc(&["build"]).output().unwrap());
    // Then resolve and execute the named dcc command.
    assert_success(&fx.dcc(&["run", "true"]).output().unwrap());
}
