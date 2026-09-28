mod common;
use common::*;

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn empty_fixture() -> Fixture {
    Fixture {
        dir: tempfile::tempdir().unwrap(),
    }
}

fn init_git(root: &std::path::Path) {
    let output = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(root)
        .output()
        .unwrap();
    assert_success(&output);
}

#[test]
fn bootstrap_initializes_a_non_git_directory() {
    let fx = empty_fixture();
    let output = fx
        .dcc(&["profile", "bootstrap", "--image=debian:bookworm-slim"])
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(
        read_json(&fx.dir.path().join(".devcontainer/devcontainer.json")),
        serde_json::json!({ "image": "debian:bookworm-slim" })
    );
    assert!(!fx.dir.path().join(".dcc").exists());
}

#[test]
fn bootstrap_initializes_without_git_on_path() {
    let fx = empty_fixture();
    let output = fx
        .dcc(&["profile", "bootstrap", "--image=debian"])
        .env("PATH", "")
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(
        read_json(&fx.dir.path().join(".devcontainer/devcontainer.json")),
        serde_json::json!({ "image": "debian" })
    );
}

#[test]
fn bootstrap_preserves_a_file_blocking_directory_creation() {
    let fx = empty_fixture();
    let path = fx.dir.path().join(".devcontainer");
    std::fs::write(&path, "preserve me").unwrap();
    for dry_run in [false, true] {
        let mut command = fx.dcc(&["profile", "bootstrap", "--image=debian"]);
        if dry_run {
            command.arg("--dry-run");
        }
        let output = command.output().unwrap();
        assert_failure(&output);
        assert_stderr_contains(&output, ".devcontainer");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserve me");
    }
}

#[test]
fn bootstrap_initializes_the_git_root_from_root_or_nested_directory() {
    for relative_cwd in ["", "src/nested"] {
        let fx = empty_fixture();
        init_git(fx.dir.path());
        let cwd = fx.dir.path().join(relative_cwd);
        std::fs::create_dir_all(&cwd).unwrap();

        let output = fx
            .dcc(&[
                "profile",
                "bootstrap",
                "-p",
                "ci",
                "--dockerfile=../Dockerfile",
            ])
            .current_dir(&cwd)
            .output()
            .unwrap();

        assert_success(&output);
        assert_eq!(
            read_json(&fx.dir.path().join(".devcontainer/ci.json")),
            serde_json::json!({ "build": { "dockerfile": "../Dockerfile" } })
        );
        if !relative_cwd.is_empty() {
            assert!(!cwd.join(".devcontainer").exists());
        }
    }
}

#[test]
fn bootstrap_prefers_an_existing_workspace_to_the_git_root() {
    let fx = empty_fixture();
    init_git(fx.dir.path());
    let workspace = fx.dir.path().join("project");
    let nested = workspace.join("src");
    std::fs::create_dir_all(workspace.join(".devcontainer")).unwrap();
    std::fs::create_dir(&nested).unwrap();

    let output = fx
        .dcc(&["profile", "bootstrap", "--image=debian"])
        .current_dir(&nested)
        .output()
        .unwrap();

    assert_success(&output);
    assert_eq!(
        read_json(&workspace.join(".devcontainer/devcontainer.json")),
        serde_json::json!({ "image": "debian" })
    );
    assert!(!fx.dir.path().join(".devcontainer").exists());
    assert!(!nested.join(".devcontainer").exists());
}

#[test]
fn bootstrap_dry_run_plans_a_missing_directory_without_creating_it() {
    for git_repo in [false, true] {
        let fx = empty_fixture();
        let cwd = if git_repo {
            init_git(fx.dir.path());
            let output = std::process::Command::new("git")
                .args([
                    "remote",
                    "add",
                    "origin",
                    "https://example.test/team/project.git",
                ])
                .current_dir(fx.dir.path())
                .output()
                .unwrap();
            assert_success(&output);
            let nested = fx.dir.path().join("src");
            std::fs::create_dir(&nested).unwrap();
            nested
        } else {
            fx.dir.path().to_path_buf()
        };
        let args = ["profile", "bootstrap", "-p", "ci", "--image=debian"];
        let output = fx
            .dcc(&args)
            .args(["--dry-run", "--format=json"])
            .current_dir(&cwd)
            .output()
            .unwrap();

        assert_success(&output);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["status"], "ok");
        assert_eq!(report["docker_invoked"], false);
        assert_eq!(
            report["config"],
            fx.dir
                .path()
                .canonicalize()
                .unwrap()
                .join(".devcontainer/ci.json")
                .to_str()
                .unwrap()
        );
        assert!(!fx.dir.path().join(".devcontainer").exists());
        assert!(!cwd.join(".devcontainer").exists());
        assert!(!fx.dir.path().join(".dcc").exists());

        assert_success(&fx.dcc(&args).current_dir(&cwd).output().unwrap());
        let id_output = fx
            .dcc(&["id", "-p", "ci", "--format=json"])
            .output()
            .unwrap();
        assert_success(&id_output);
        let id: serde_json::Value = serde_json::from_slice(&id_output.stdout).unwrap();
        assert_eq!(report["container_id"], id["container_id"]);
    }
}

#[test]
fn bootstrap_invalid_requests_leave_a_fresh_directory_untouched() {
    let fx = empty_fixture();
    for (args, diagnostic) in [
        (vec!["--image="], "non-empty"),
        (vec!["--dockerfile="], "non-empty"),
        (
            vec!["--image=debian", "-p", "nested/profile"],
            "profile name",
        ),
        (vec!["--extends=missing"], "missing.json"),
        (
            vec!["--image=debian", "--dockerfile=Dockerfile"],
            "cannot be used with",
        ),
    ] {
        let output = fx
            .dcc(&["profile", "bootstrap"])
            .args(args)
            .output()
            .unwrap();
        assert_failure(&output);
        assert_stderr_contains(&output, diagnostic);
        assert!(!fx.dir.path().join(".devcontainer").exists());
        assert!(!fx.dir.path().join(".dcc").exists());
    }
}

#[test]
fn other_commands_still_require_a_devcontainer_directory_in_git_repos() {
    let fx = empty_fixture();
    init_git(fx.dir.path());
    for args in [
        vec!["build", "--dry-run"],
        vec!["profile", "list"],
        vec!["id"],
    ] {
        let output = fx.dcc(&args).output().unwrap();
        assert_failure(&output);
        assert_stderr_contains(&output, "could not find `.devcontainer/`");
        assert!(!fx.dir.path().join(".devcontainer").exists());
    }
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
