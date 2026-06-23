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
        .dcc(&["--strict", "exec", "echo", "OK"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unrecognized field 'name'"),
        "`--strict exec` should accept devcontainer `name`\nstderr: {stderr}"
    );
}

#[test]
fn default_mode_warns_on_unknown_fields_but_does_not_fail_early() {
    let fx = Fixture::new();
    fx.write_config(
        "devcontainer.json",
        r#"{ "image": "rust:1", "unknownField": "value" }"#,
    );
    // Without --strict: should not fail due to the unknown field.
    // It may still fail (Docker not available), but the failure should
    // NOT be about strict mode / unknown field being fatal.
    // Set RUST_LOG=warn so tracing::warn! output appears in stderr.
    let output = fx.dcc(&["build"]).env("RUST_LOG", "warn").output().unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
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
fn dash_dash_not_rejected_by_arg_parser() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    // `dcc run --` should be accepted syntactically (may fail due to missing Docker)
    let output = fx.dcc(&["run", "--"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "`--` should be valid CLI syntax, not rejected by clap\nstderr: {stderr}"
    );
}

#[test]
fn positional_args_after_run_accepted() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    let output = fx.dcc(&["run", "/bin/true"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "positional args after `run` should be accepted by clap\nstderr: {stderr}"
    );
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
    // `--skip-lifecycle` must precede the trailing command. It may still fail (no
    // Docker daemon), but clap must not reject the flag as unexpected.
    let output = fx
        .dcc(&["exec", "--skip-lifecycle", "/bin/true"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument") && !stderr.contains("--skip-lifecycle"),
        "`exec --skip-lifecycle` should be accepted by clap\nstderr: {stderr}"
    );
}

#[test]
fn debug_flag_accepted_by_exec_and_run() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    // `--debug` must precede the trailing command on exec. Both may still fail (no
    // Docker daemon), but clap must not reject the flag as unexpected.
    for args in [
        ["exec", "--debug", "/bin/true"].as_slice(),
        ["run", "--debug", "noop"].as_slice(),
    ] {
        let output = fx.dcc(args).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("unexpected argument") && !stderr.contains("--debug"),
            "`{args:?}` should be accepted by clap\nstderr: {stderr}"
        );
    }
}

// Tests below require a live Docker daemon — skipped in CI
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
fn dash_dash_passes_command_override() {
    let fx = Fixture::new();
    fx.write_config("devcontainer.json", r#"{ "image": "rust:1" }"#);
    // Build first
    assert_success(&fx.dcc(&["build"]).output().unwrap());
    // Then run with override
    assert_success(&fx.dcc(&["run", "--", "/bin/true"]).output().unwrap());
}
