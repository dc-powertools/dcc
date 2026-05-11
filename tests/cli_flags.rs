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
