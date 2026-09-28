#![cfg(unix)]

mod common;
use common::*;

use std::{
    ffi::OsString,
    path::PathBuf,
    process::{Command, Output},
};

const FAKE_DOCKER: &str = r#"#!/bin/sh
set -eu
{
    printf '%s\n' __DCC_FAKE_CALL__
    for argument in "$@"; do
        printf '%s\n' "$argument"
    done
    printf '%s\n' __DCC_FAKE_END__
} >> "$DCC_FAKE_DOCKER_LOG"

command_name=${1-}
if [ "$command_name" = version ]; then printf '%s\n' "${DCC_FAKE_ENGINE_VERSION-28.0.0}"; exit 0; fi
if [ "$command_name" = network ]; then printf '%s\n' '[{"Driver":"bridge","Options":{}}]'; exit 0; fi
if [ "$command_name" = build ]; then
    cat >/dev/null
    exit 0
fi

if [ "$command_name" = image ] && [ "${2-}" = inspect ]; then
    if [ "${3-}" != --format ]; then
        exit 0
    fi
    case "${4-}" in
        *'.Id'*) printf '%s\n' 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' ;;
        *dcc.version*) printf '%s\n' "${DCC_FAKE_VERSION_LABEL-<no value>}" ;;
        *devcontainer.metadata*) printf '%s\n' "${DCC_FAKE_METADATA-<no value>}" ;;
        *Config.Env*) printf '%s\n' "${DCC_FAKE_IMAGE_ENV-[]}" ;;
        *dcc.seed*) printf '%s\n' '<no value>' ;;
        *) printf '%s\n' '<no value>' ;;
    esac
    exit 0
fi

if [ "$command_name" = ps ]; then
    for argument in "$@"; do
        if [ "$argument" = --all ]; then
            if [ -f "$DCC_FAKE_DOCKER_STATE.inspect" ]; then printf '%s\n' bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb; fi
            exit 0
        fi
    done
    for argument in "$@"; do
        case "$argument" in
            *'{{.ID}}'*)
                if [ -f "$DCC_FAKE_DOCKER_STATE.inspect" ] && [ -s "$DCC_FAKE_DOCKER_STATE" ]; then
                    printf 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\truntime\n'
                fi
                exit 0 ;;
        esac
    done
    profile_status=false
    for argument in "$@"; do
        case "$argument" in
            *dcc.container_role*) profile_status=true ;;
        esac
    done
    if [ "$profile_status" = true ]; then
        if [ -n "${DCC_FAKE_PROFILE_PS_FAIL-}" ]; then
            printf '%s\n' 'fake Docker status failure' >&2
            exit 42
        fi
        if [ -f "$DCC_FAKE_PROFILE_STATUS" ]; then
            cat "$DCC_FAKE_PROFILE_STATUS"
        fi
        exit 0
    fi
    if [ -s "$DCC_FAKE_DOCKER_STATE" ]; then
        cat "$DCC_FAKE_DOCKER_STATE"
        printf '\n'
    fi
    exit 0
fi

if [ "$command_name" = container ] && [ "${2-}" = inspect ]; then
    if [ -f "$DCC_FAKE_DOCKER_STATE.inspect" ]; then cat "$DCC_FAKE_DOCKER_STATE.inspect"; exit 0; fi
    echo 'Error: No such container' >&2
    exit 1
fi
if [ "$command_name" = create ]; then
    previous= name= identity= token= payload=
    for argument in "$@"; do
        case "$previous" in
            --name) name=$argument ;;
            --label) case "$argument" in dcc.container_id=*) identity=${argument#*=} ;; dcc.launch_token=*) token=${argument#*=} ;; esac ;;
            --mount) case "$argument" in *instances*payload*) payload=${argument#*source=}; payload=${payload%%,*} ;; esac ;;
        esac
        previous=$argument
    done
    printf '%s' "$name" > "$DCC_FAKE_DOCKER_STATE"
    printf '%s' "$payload" > "$DCC_FAKE_DOCKER_STATE.payload"
    bindings=${DCC_FAKE_BINDINGS-'{}'}
    printf '[{"Id":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","Image":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","Config":{"Labels":{"dcc.container_id":"%s","dcc.container_role":"runtime","dcc.launch_token":"%s"}},"State":{"Status":"created","StartedAt":"0001-01-01T00:00:00Z"},"NetworkSettings":{"Ports":%s}}]\n' "$identity" "$token" "$bindings" > "$DCC_FAKE_DOCKER_STATE.inspect"
    printf '%s\n' bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
    exit 0
fi
if [ "$command_name" = start ]; then
    if [ -n "${DCC_FAKE_START_FAIL-}" ] && [ ! -f "$DCC_FAKE_DOCKER_STATE.failed" ]; then
        touch "$DCC_FAKE_DOCKER_STATE.failed"
        if [ "$DCC_FAKE_START_FAIL" = uncertain ]; then
            sed 's/"created"/"exited"/;s/0001-01-01/2026-09-28/' "$DCC_FAKE_DOCKER_STATE.inspect" > "$DCC_FAKE_DOCKER_STATE.tmp"
            mv "$DCC_FAKE_DOCKER_STATE.tmp" "$DCC_FAKE_DOCKER_STATE.inspect"
        fi
        echo 'simulated start failure' >&2
        exit 1
    fi
    sed 's/"created"/"running"/;s/0001-01-01/2026-09-28/' "$DCC_FAKE_DOCKER_STATE.inspect" > "$DCC_FAKE_DOCKER_STATE.tmp"
    mv "$DCC_FAKE_DOCKER_STATE.tmp" "$DCC_FAKE_DOCKER_STATE.inspect"
    exit 0
fi
if [ "$command_name" = rm ]; then rm -f "$DCC_FAKE_DOCKER_STATE.inspect" "$DCC_FAKE_DOCKER_STATE"; exit 0; fi

if [ "$command_name" = inspect ]; then
    if [ "${2-}" = --format ] && [ "${3-}" = '{{json .Mounts}}' ]; then
        [ -z "${DCC_FAKE_MOUNT_INSPECT_FAIL-}" ] || exit 1
        printf '[{"Source":"%s"}]\n' "$(cat "$DCC_FAKE_DOCKER_STATE.payload")"
        exit 0
    fi
    if [ -s "$DCC_FAKE_DOCKER_STATE" ]; then
        printf '%s\n' true
        exit 0
    fi
    exit 1
fi

if [ "$command_name" = run ]; then
    cat >/dev/null
    previous=
    name=
    for argument in "$@"; do
        if [ "$previous" = --name ]; then
            name=$argument
        fi
        previous=$argument
    done
    if [ -n "$name" ]; then
        printf '%s' "$name" > "$DCC_FAKE_DOCKER_STATE"
    fi
    exit 0
fi

if [ "$command_name" = exec ]; then
    previous=
    for argument in "$@"; do
        case "$argument" in
            stop|stop-now) rm -f "$DCC_FAKE_DOCKER_STATE" ;;
            snapshot) cat "$(cat "$DCC_FAKE_DOCKER_STATE.payload")/snapshot.json"; exit 0 ;;
            relay-status) printf '%s\n' "${DCC_FAKE_RELAY_STATUS-}"; exit "${DCC_FAKE_RELAY_CODE-0}" ;;
        esac
        if [ "$previous" = verify-config ]; then
            [ "$argument" = "$(cat "$(cat "$DCC_FAKE_DOCKER_STATE.payload")/fingerprint")" ] || exit 3
        fi
        previous=$argument
    done
    exit 0
fi

if [ "$command_name" = stop ] || [ "$command_name" = kill ]; then
    rm -f "$DCC_FAKE_DOCKER_STATE"
    exit 0
fi

exit 0
"#;

struct FakeDockerFixture {
    fx: Fixture,
    path: OsString,
    log: PathBuf,
    state: PathBuf,
    profile_status: PathBuf,
}

impl FakeDockerFixture {
    fn new(config: &str) -> Self {
        use std::os::unix::fs::PermissionsExt as _;

        let fx = Fixture::new();
        fx.write_config("devcontainer.json", config);
        let bin = fx.dir.path().join("fake-bin");
        std::fs::create_dir(&bin).unwrap();
        let docker = bin.join("docker");
        std::fs::write(&docker, FAKE_DOCKER).unwrap();
        let mut permissions = std::fs::metadata(&docker).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&docker, permissions).unwrap();

        let mut paths = vec![bin];
        if let Some(existing) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        let path = std::env::join_paths(paths).unwrap();
        let log = fx.dir.path().join("docker-calls.log");
        let state = fx.dir.path().join("docker-state");
        let profile_status = fx.dir.path().join("profile-status");
        Self {
            fx,
            path,
            log,
            state,
            profile_status,
        }
    }

    fn dcc(&self, args: &[&str]) -> Command {
        let mut command = self.fx.dcc(args);
        command
            .env("PATH", &self.path)
            .env("DCC_FAKE_DOCKER_LOG", &self.log)
            .env("DCC_FAKE_DOCKER_STATE", &self.state)
            .env("DCC_FAKE_PROFILE_STATUS", &self.profile_status);
        command
    }

    fn output(&self, args: &[&str], version: Option<&str>) -> Output {
        let mut command = self.dcc(args);
        if let Some(version) = version {
            command.env("DCC_FAKE_VERSION_LABEL", version);
        }
        command.output().unwrap()
    }

    fn output_with_image_env(
        &self,
        args: &[&str],
        version: Option<&str>,
        image_env: &str,
    ) -> Output {
        let mut command = self.dcc(args);
        if let Some(version) = version {
            command.env("DCC_FAKE_VERSION_LABEL", version);
        }
        command.env("DCC_FAKE_IMAGE_ENV", image_env);
        command.output().unwrap()
    }

    fn output_with_metadata(&self, args: &[&str], version: Option<&str>, metadata: &str) -> Output {
        let mut command = self.dcc(args);
        if let Some(version) = version {
            command.env("DCC_FAKE_VERSION_LABEL", version);
        }
        command.env("DCC_FAKE_METADATA", metadata);
        command.output().unwrap()
    }

    fn set_running(&self, name: &str) {
        std::fs::write(&self.state, name).unwrap();
    }

    fn set_profile_status(&self, records: &str) {
        std::fs::write(&self.profile_status, records).unwrap();
    }

    fn profile_id(&self, profile: &str) -> String {
        let output = self.dcc(&["--profile", profile, "id"]).output().unwrap();
        assert_success(&output);
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn calls(&self) -> Vec<Vec<String>> {
        let contents = std::fs::read_to_string(&self.log).unwrap_or_default();
        let mut calls = Vec::new();
        let mut current = None;
        for line in contents.lines() {
            match line {
                "__DCC_FAKE_CALL__" => current = Some(Vec::new()),
                "__DCC_FAKE_END__" => calls.push(current.take().expect("call start marker")),
                argument => current
                    .as_mut()
                    .expect("argument outside call markers")
                    .push(argument.to_string()),
            }
        }
        assert!(current.is_none(), "unterminated fake Docker call log");
        calls
    }
}

fn root_image_config() -> &'static str {
    r#"{ "image": "debian:bookworm-slim", "containerUser": "root" }"#
}

fn compatible_patch_version() -> String {
    let mut parts = env!("CARGO_PKG_VERSION").split('.');
    let major = parts.next().unwrap();
    let minor = parts.next().unwrap();
    let patch = parts.next().unwrap().parse::<u64>().unwrap() + 1;
    format!("{major}.{minor}.{patch}")
}

fn incompatible_major_version() -> String {
    let major = env!("CARGO_PKG_VERSION")
        .split('.')
        .next()
        .unwrap()
        .parse::<u64>()
        .unwrap()
        + 1;
    format!("{major}.0.0")
}

fn incompatible_minor_version() -> String {
    let mut parts = env!("CARGO_PKG_VERSION").split('.');
    let major = parts.next().unwrap();
    let minor = parts.next().unwrap().parse::<u64>().unwrap() + 1;
    format!("{major}.{minor}.0")
}

fn contains_pair(call: &[String], flag: &str, value: &str) -> bool {
    call.windows(2)
        .any(|pair| pair[0] == flag && pair[1] == value)
}

fn assert_resource_limits_before_image(run: &[String], memory_value: &str, cpus_value: &str) {
    assert!(contains_pair(run, "--memory", memory_value));
    assert!(contains_pair(run, "--cpus", cpus_value));

    let mode = run.iter().position(|arg| arg == "--mode").unwrap();
    let image = mode - 1;
    let memory = run.iter().position(|arg| arg == "--memory").unwrap();
    let cpus = run.iter().position(|arg| arg == "--cpus").unwrap();
    assert!(memory < image && cpus < image);
    assert!(
        !run[image].starts_with('-'),
        "image must precede supervisor arguments: {run:?}"
    );
}

fn build_calls(calls: &[Vec<String>]) -> Vec<&Vec<String>> {
    calls
        .iter()
        .filter(|call| call.first().is_some_and(|arg| arg == "build"))
        .collect()
}

fn run_call(calls: &[Vec<String>]) -> &Vec<String> {
    calls
        .iter()
        .find(|call| {
            call.first()
                .is_some_and(|arg| arg == "run" || arg == "create")
                && call.iter().any(|arg| arg.ends_with("/dcc-supervisor"))
        })
        .expect("expected a docker run call")
}

fn profile_status_calls(calls: &[Vec<String>]) -> Vec<&Vec<String>> {
    calls
        .iter()
        .filter(|call| {
            call.first().is_some_and(|arg| arg == "ps")
                && call.iter().any(|arg| arg.contains("dcc.container_role"))
        })
        .collect()
}

fn tagged_build<'a>(calls: &'a [&Vec<String>], suffix: &str) -> &'a Vec<String> {
    calls
        .iter()
        .copied()
        .find(|call| {
            call.windows(2)
                .any(|pair| pair[0] == "--tag" && pair[1].ends_with(suffix))
        })
        .unwrap_or_else(|| panic!("no build tagged with suffix {suffix}: {calls:?}"))
}

#[test]
fn profile_list_queries_docker_once_and_marks_only_runtime_profiles() {
    let fx = FakeDockerFixture::new(root_image_config());
    fx.fx.write_config("alpha.json", "{}");
    fx.fx.write_config("ci.json", "{}");
    let alpha_id = fx.profile_id("alpha");
    let ci_id = fx.profile_id("ci");
    let default_id = fx.profile_id("devcontainer");
    fx.set_profile_status(&format!(
        "{alpha_id}\tbuild-prep\t{alpha_id}-build-prep\n\
         {ci_id}\truntime\tci-container\n\
         {default_id}\truntime\tdefault-container\n"
    ));

    let output = fx.dcc(&["profile", "list"]).output().unwrap();
    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "alpha\nci [running]\ndevcontainer (default) [running]\n"
    );
    assert!(output.stderr.is_empty());

    let calls = fx.calls();
    assert_eq!(calls.len(), 1, "expected one Docker call: {calls:?}");
    let status_calls = profile_status_calls(&calls);
    assert_eq!(status_calls.len(), 1, "missing status query: {calls:?}");
    assert_eq!(
        status_calls[0],
        &[
            "ps",
            "--filter",
            "label=dcc.container_id",
            "--format",
            r#"{{.Label "dcc.container_id"}}\t{{.Label "dcc.container_role"}}\t{{.Names}}"#,
        ]
    );
}

#[test]
fn profile_list_json_reports_true_and_false_from_one_snapshot() {
    let fx = FakeDockerFixture::new(root_image_config());
    fx.fx.write_config("ci.json", "{}");
    let ci_id = fx.profile_id("ci");
    fx.set_profile_status(&format!("{ci_id}\truntime\tci-container\n"));

    let output = fx
        .dcc(&["profile", "list", "--format", "json"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({
            "profiles": [
                {
                    "name": "ci",
                    "config": ".devcontainer/ci.json",
                    "default": false,
                    "running": true
                },
                {
                    "name": "devcontainer",
                    "config": ".devcontainer/devcontainer.json",
                    "default": true,
                    "running": false
                }
            ]
        })
    );
    assert!(output.stderr.is_empty());
    let calls = fx.calls();
    assert_eq!(calls.len(), 1, "expected one Docker call: {calls:?}");
    assert_eq!(profile_status_calls(&calls).len(), 1);
}

#[test]
fn profile_list_empty_discovery_does_not_query_docker() {
    let fx = FakeDockerFixture::new(root_image_config());
    std::fs::remove_file(fx.fx.dir.path().join(".devcontainer/devcontainer.json")).unwrap();

    let output = fx
        .dcc(&["profile", "list", "--format", "json"])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "{\n  \"profiles\": []\n}\n"
    );
    assert!(output.stderr.is_empty());
    assert!(fx.calls().is_empty(), "empty discovery invoked Docker");
}

#[test]
fn profile_list_dry_run_skips_docker_and_reports_unknown_status() {
    let fx = FakeDockerFixture::new(root_image_config());
    fx.fx.write_config("ci.json", "{}");

    let output = fx
        .dcc(&["--dry-run", "--format", "json", "profile", "list"])
        .output()
        .unwrap();
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["profiles"][0]["running"], serde_json::Value::Null);
    assert_eq!(report["profiles"][1]["running"], serde_json::Value::Null);
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "warning: profile running status is unknown because --dry-run skipped the Docker status query\n"
    );
    assert!(
        fx.calls().is_empty(),
        "profile list --dry-run invoked Docker"
    );
}

#[test]
fn profile_list_nonzero_docker_status_degrades_to_unknown_once() {
    let fx = FakeDockerFixture::new(root_image_config());
    fx.fx.write_config("ci.json", "{}");

    let output = fx
        .dcc(&["--format", "json", "profile", "list"])
        .env("DCC_FAKE_PROFILE_PS_FAIL", "1")
        .output()
        .unwrap();
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["profiles"][0]["running"], serde_json::Value::Null);
    assert_eq!(report["profiles"][1]["running"], serde_json::Value::Null);
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "warning: profile running status is unknown because Docker status could not be queried\n"
    );
    assert_eq!(profile_status_calls(&fx.calls()).len(), 1);
}

#[test]
fn profile_list_malformed_docker_status_degrades_to_unknown_once() {
    let fx = FakeDockerFixture::new(root_image_config());
    fx.fx.write_config("ci.json", "{}");
    fx.set_profile_status("malformed-record\n");

    let output = fx.dcc(&["profile", "list"]).output().unwrap();
    assert_success(&output);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "ci\ndevcontainer (default)\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "warning: profile running status is unknown because Docker status could not be queried\n"
    );
    assert_eq!(profile_status_calls(&fx.calls()).len(), 1);
}

#[test]
fn profile_list_unknown_role_is_unknown_unless_runtime_also_exists() {
    let fx = FakeDockerFixture::new(root_image_config());
    fx.fx.write_config("ci.json", "{}");
    let ci_id = fx.profile_id("ci");
    let default_id = fx.profile_id("devcontainer");
    fx.set_profile_status(&format!(
        "{ci_id}\tfuture-role\tci-future\n\
         {default_id}\tfuture-role\tdefault-future\n\
         {default_id}\truntime\tdefault-runtime\n"
    ));

    let output = fx
        .dcc(&["--debug", "--format", "json", "profile", "list"])
        .output()
        .unwrap();
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["profiles"][0]["running"], serde_json::Value::Null);
    assert_eq!(report["profiles"][1]["running"], true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr
            .matches("warning: profile running status is unknown for one or more profiles because Docker reported an unrecognized dcc container role")
            .count(),
        1
    );
    assert!(stderr.contains(
        "dcc debug: profile `ci` has running container `ci-future` with unrecognized role `future-role`"
    ));
    assert!(!stderr.contains("profile `devcontainer` has running container"));
}

#[test]
fn runtime_refuses_missing_version_label_before_container_work() {
    let fx = FakeDockerFixture::new(root_image_config());
    let output = fx.output(&["start"], None);
    assert_failure(&output);
    assert_stderr_contains(&output, "does not record the dcc version");
    assert_stderr_contains(&output, "`dcc build`");
    assert!(
        !fx.calls()
            .iter()
            .any(|call| call.first().is_some_and(|arg| arg == "run")),
        "runtime work must not begin after a missing version label"
    );
}

#[test]
fn runtime_refuses_major_and_minor_incompatibility_with_rebuild_instruction() {
    for incompatible in [incompatible_major_version(), incompatible_minor_version()] {
        let fx = FakeDockerFixture::new(root_image_config());
        let output = fx.output(&["--strict", "start"], Some(&incompatible));
        assert_failure(&output);
        assert_stderr_contains(&output, "is incompatible with current dcc");
        assert_stderr_contains(&output, "`dcc --strict build`");
        assert!(
            !fx.calls()
                .iter()
                .any(|call| call.first().is_some_and(|arg| arg == "run")),
            "runtime work must not begin after incompatible label {incompatible}"
        );
    }
}

#[test]
fn patch_compatible_version_reaches_runtime_container_creation() {
    let fx = FakeDockerFixture::new(root_image_config());
    let compatible = compatible_patch_version();
    let output = fx.output(&["start"], Some(&compatible));
    assert_success(&output);
    let calls = fx.calls();
    let run = run_call(&calls);
    assert!(run.iter().any(|arg| arg == "--entrypoint"));
    assert!(run.iter().any(|arg| arg == "--mode"));
    assert!(contains_pair(run, "--label", "dcc.container_role=runtime"));
}

#[test]
fn build_preparation_container_has_build_prep_role_label() {
    let fx = FakeDockerFixture::new(
        r#"{
            "image": "debian:bookworm-slim",
            "containerUser": "root",
            "postCreateCommand": "true"
        }"#,
    );
    let output = fx.output(&["build"], None);
    assert_success(&output);

    let calls = fx.calls();
    let prep = calls
        .iter()
        .find(|call| {
            call.first().is_some_and(|arg| arg == "run")
                && call
                    .windows(2)
                    .any(|pair| pair[0] == "--name" && pair[1].ends_with("-build-prep"))
        })
        .expect("expected build-preparation docker run");
    assert!(contains_pair(
        prep,
        "--label",
        "dcc.container_role=build-prep"
    ));
}

#[test]
fn stop_is_best_effort_when_version_label_is_missing() {
    let fx = FakeDockerFixture::new(root_image_config());
    fx.set_running("fake-running-container");
    let output = fx.output(&["stop"], None);
    assert_success(&output);
    let calls = fx.calls();
    assert!(calls.iter().any(|call| {
        call.first().is_some_and(|arg| arg == "exec") && call.iter().any(|arg| arg == "stop")
    }));
    assert!(!fx.state.exists(), "fake running container was not stopped");
}

#[test]
fn no_cache_pulls_upstream_image_profile_base() {
    let fx = FakeDockerFixture::new(root_image_config());
    let output = fx.output(&["build", "--no-cache"], None);
    assert_success(&output);
    let calls = fx.calls();
    let builds = build_calls(&calls);
    assert_eq!(builds.len(), 1, "unexpected build calls: {builds:?}");
    assert!(builds[0].iter().any(|arg| arg == "--no-cache"));
    assert!(builds[0].iter().any(|arg| arg == "--pull"));
}

#[test]
fn no_cache_pulls_official_source_but_not_generated_intermediate() {
    let fx = FakeDockerFixture::new(
        r#"{
            "build": { "dockerfile": "Dockerfile", "context": "." },
            "containerUser": "root"
        }"#,
    );
    fx.fx
        .write_config("Dockerfile", "FROM debian:bookworm-slim\n");
    let output = fx.output(&["build", "--no-cache"], None);
    assert_success(&output);

    let calls = fx.calls();
    let builds = build_calls(&calls);
    assert_eq!(builds.len(), 2, "unexpected build calls: {builds:?}");
    let upstream = tagged_build(&builds, "-base");
    let generated = builds
        .iter()
        .copied()
        .find(|call| !std::ptr::eq(*call, upstream))
        .expect("generated dcc build");
    assert!(upstream.iter().any(|arg| arg == "--no-cache"));
    assert!(upstream.iter().any(|arg| arg == "--pull"));
    assert!(generated.iter().any(|arg| arg == "--no-cache"));
    assert!(
        !generated.iter().any(|arg| arg == "--pull"),
        "generated local intermediate must not be pulled: {generated:?}"
    );
}

#[test]
fn explicit_resource_limits_reach_docker_run_before_image_arguments() {
    let fx = FakeDockerFixture::new(root_image_config());
    let compatible = compatible_patch_version();
    let output = fx.output(
        &["start", "--memory", "768m", "--cpus", "1.25"],
        Some(&compatible),
    );
    assert_success(&output);
    let calls = fx.calls();
    let run = run_call(&calls);
    assert_resource_limits_before_image(run, "768m", "1.25");
}

#[test]
fn default_resource_limits_reach_every_runtime_container_creation_path() {
    let config = r#"{
        "image": "debian:bookworm-slim",
        "containerUser": "root",
        "customizations": { "dcc": { "commands": { "test": "true" } } }
    }"#;
    let cases: &[(&str, &[&str])] = &[
        ("start", &["start"]),
        ("exec", &["exec", "--keep", "true"]),
        ("attach", &["attach", "--keep", "/bin/true"]),
        ("run", &["run", "--keep", "test"]),
    ];

    for (name, args) in cases {
        let fx = FakeDockerFixture::new(config);
        let compatible = compatible_patch_version();
        let output = fx.output(args, Some(&compatible));
        assert_success(&output);
        let calls = fx.calls();
        let run = run_call(&calls);
        assert_resource_limits_before_image(run, "4g", "2");
        assert_eq!(
            run.iter().filter(|arg| *arg == "--memory").count(),
            1,
            "{name} emitted an unexpected memory flag count: {run:?}"
        );
        assert_eq!(
            run.iter().filter(|arg| *arg == "--cpus").count(),
            1,
            "{name} emitted an unexpected CPU flag count: {run:?}"
        );
    }
}

#[test]
fn a_single_explicit_resource_override_retains_the_other_default() {
    let compatible = compatible_patch_version();
    for (args, expected_memory, expected_cpus) in [
        (&["start", "--memory", "768m"][..], "768m", "2"),
        (&["start", "--cpus", "1.25"][..], "4g", "1.25"),
    ] {
        let fx = FakeDockerFixture::new(root_image_config());
        let output = fx.output(args, Some(&compatible));
        assert_success(&output);
        let calls = fx.calls();
        assert_resource_limits_before_image(run_call(&calls), expected_memory, expected_cpus);
    }
}

#[test]
fn reserved_dcc_run_arg_labels_are_rejected_before_container_creation() {
    let cases = [
        (
            r#"["--label", "dcc.container_id=spoofed"]"#,
            "dcc.container_id",
        ),
        (
            r#"["--label=dcc.container_role=spoofed"]"#,
            "dcc.container_role",
        ),
    ];

    for (run_args, key) in cases {
        let config = format!(
            r#"{{
                "image": "debian:bookworm-slim",
                "containerUser": "root",
                "runArgs": {run_args}
            }}"#
        );
        let fx = FakeDockerFixture::new(&config);
        let compatible = compatible_patch_version();
        let output = fx.output(&["start", "--allow-unsafe-runtime"], Some(&compatible));
        assert_failure(&output);
        assert_stderr_contains(
            &output,
            &format!("runArgs label `{key}` is reserved for dcc container lifecycle metadata"),
        );
        assert!(
            !fx.calls().iter().any(|call| {
                call.first()
                    .is_some_and(|arg| arg == "run" || arg == "create")
                    && call.iter().any(|arg| arg.ends_with("/dcc-supervisor"))
            }),
            "reserved label {key} reached container creation"
        );
    }
}

#[test]
fn unrelated_run_arg_label_remains_allowed() {
    let fx = FakeDockerFixture::new(
        r#"{
            "image": "debian:bookworm-slim",
            "containerUser": "root",
            "runArgs": ["--label=example.owner=test"]
        }"#,
    );
    let compatible = compatible_patch_version();
    let output = fx.output(&["start"], Some(&compatible));
    assert_success(&output);
    let calls = fx.calls();
    assert!(run_call(&calls)
        .iter()
        .any(|arg| arg == "--label=example.owner=test"));
}

#[test]
fn missing_container_env_without_default_fails_in_every_runtime_consumer() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            r#"{"image":"debian:bookworm-slim","containerUser":"root","workspaceFolder":"${containerEnv:MISSING}"}"#,
            "workspaceFolder",
            &["start"],
        ),
        (
            r#"{"image":"debian:bookworm-slim","containerUser":"root","runArgs":["--label=value=${containerEnv:MISSING}"]}"#,
            "runArgs[0]",
            &["start"],
        ),
        (
            r#"{"image":"debian:bookworm-slim","containerUser":"root","mounts":["type=volume,target=${containerEnv:MISSING}"]}"#,
            "mount 0",
            &["start"],
        ),
        (
            r#"{"image":"debian:bookworm-slim","containerUser":"root","remoteEnv":{"REQUIRED":"${containerEnv:MISSING}"}}"#,
            "remoteEnv `REQUIRED`",
            &["start"],
        ),
        (
            r#"{"image":"debian:bookworm-slim","containerUser":"root","customizations":{"dcc":{"state":["${containerEnv:MISSING}"]}}}"#,
            "customizations.dcc.state path",
            &["start"],
        ),
        (
            r#"{"image":"debian:bookworm-slim","containerUser":"root","postStartCommand":"echo ${containerEnv:MISSING}"}"#,
            "startup hook scripts",
            &["start"],
        ),
        (
            root_image_config(),
            "container command argument 0",
            &["exec", "${containerEnv:MISSING}"],
        ),
    ];
    let compatible = compatible_patch_version();

    for (config, context, args) in cases {
        let fx = FakeDockerFixture::new(config);
        let output = fx.output(args, Some(&compatible));
        assert_failure(&output);
        assert_stderr_contains(&output, context);
        assert_stderr_contains(&output, "variable `MISSING` is missing");
        assert_stderr_contains(&output, "${containerEnv:MISSING}");
        assert!(
            !fx.calls().iter().any(|call| {
                call.first()
                    .is_some_and(|arg| arg == "run" || arg == "create")
                    && call.iter().any(|arg| arg.ends_with("/dcc-supervisor"))
            }),
            "{context} failure must occur before profile container creation"
        );
    }
}

#[test]
fn container_env_default_and_present_empty_reach_runtime_environment() {
    let config = r#"{
        "image":"debian:bookworm-slim",
        "containerUser":"root",
        "remoteEnv":{
            "ABSENT":"${containerEnv:MISSING:fallback}",
            "EMPTY":"${containerEnv:EMPTY:fallback}"
        }
    }"#;
    let fx = FakeDockerFixture::new(config);
    let compatible = compatible_patch_version();
    let output = fx.output_with_image_env(&["start"], Some(&compatible), r#"["EMPTY="]"#);
    assert_success(&output);

    let calls = fx.calls();
    let run = calls
        .iter()
        .find(|call| {
            call.first()
                .is_some_and(|arg| arg == "run" || arg == "create")
                && call.iter().any(|arg| arg.ends_with("/dcc-supervisor"))
        })
        .expect("expected profile container creation");
    assert!(contains_pair(run, "-e", "ABSENT=fallback"));
    assert!(contains_pair(run, "-e", "EMPTY="));
}

#[test]
fn missing_container_env_fails_build_preparation_lifecycle_hook() {
    let config = r#"{
        "image":"debian:bookworm-slim",
        "containerUser":"root",
        "postCreateCommand":"echo ${containerEnv:MISSING}"
    }"#;
    let fx = FakeDockerFixture::new(config);
    let output = fx.output(&["build", "--refresh-only"], None);
    assert_failure(&output);
    assert_stderr_contains(&output, "postCreateCommand from project");
    assert_stderr_contains(&output, "variable `MISSING` is missing");
}

#[test]
fn project_and_feature_mount_forms_reach_docker_with_readonly_preserved() {
    let fx = FakeDockerFixture::new(
        r#"{
        "image":"debian:bookworm-slim", "containerUser":"root",
        "mounts":[
            "type=volume,source=project-data,target=/project-data,readonly",
            {"type":"volume","target":"/project-anonymous"}
        ]
    }"#,
    );
    let metadata = r#"[{"id":"feat","mounts":[
        "type=volume,source=feature-data,target=/feature-data,readonly",
        {"type":"volume","target":"/feature-anonymous"}
    ]}]"#;
    let output = fx.output_with_metadata(&["start"], Some(&compatible_patch_version()), metadata);
    assert_success(&output);
    let calls = fx.calls();
    let run = calls
        .iter()
        .find(|call| {
            call.first()
                .is_some_and(|arg| arg == "run" || arg == "create")
                && call.iter().any(|arg| arg.ends_with("/dcc-supervisor"))
        })
        .expect("expected profile container creation");
    for mount in [
        "type=volume,source=project-data,target=/project-data,readonly",
        "type=volume,source=feature-data,target=/feature-data,readonly",
        "type=volume,target=/project-anonymous",
        "type=volume,target=/feature-anonymous",
    ] {
        assert!(contains_pair(run, "--mount", mount), "{run:?}");
    }
}

#[test]
fn missing_container_env_without_default_fails_in_feature_consumers() {
    let cases = [
        (
            r#"[{"id":"feat","remoteEnv":{"REQUIRED":"${containerEnv:MISSING}"}}]"#,
            "remoteEnv `REQUIRED`",
        ),
        (
            r#"[{"id":"feat","mounts":[{"type":"volume","target":"${containerEnv:MISSING}"}]}]"#,
            "mount 0",
        ),
        (
            r#"[{"id":"feat","customizations":{"dcc":{"state":["${containerEnv:MISSING}"]}}}]"#,
            "customizations.dcc.state path",
        ),
        (
            r#"[{"id":"feat","postStartCommand":"echo ${containerEnv:MISSING}"}]"#,
            "postStartCommand from feature `feat`",
        ),
    ];
    let compatible = compatible_patch_version();

    for (metadata, context) in cases {
        let fx = FakeDockerFixture::new(root_image_config());
        let output = fx.output_with_metadata(&["start"], Some(&compatible), metadata);
        assert_failure(&output);
        assert_stderr_contains(&output, context);
        assert_stderr_contains(&output, "variable `MISSING` is missing");
    }
}

#[test]
fn reuse_warns_and_defers_config_edits_without_recreating_or_rewriting_assets() {
    let fx = FakeDockerFixture::new(
        r#"{"image":"debian","containerUser":"root","postAttachCommand":"echo old-hook","customizations":{"dcc":{"commands":{"check":"echo old-command"}}}}"#,
    );
    let version = compatible_patch_version();
    assert_success(&fx.output(&["start"], Some(&version)));
    let payload = std::fs::read_to_string(fx.state.with_extension("payload")).unwrap();
    let snapshot = std::fs::read(format!("{payload}/snapshot.json")).unwrap();
    fx.fx.write_config("devcontainer.json", r#"{"image":"debian","containerUser":"different","postAttachCommand":"echo new-hook","forwardPorts":[4173],"customizations":{"dcc":{"commands":{"check":"echo new-command"}}}}"#);
    let output = fx.output(&["run", "--keep", "check"], Some(&version));
    assert_success(&output);
    assert_stderr_contains(&output, "configuration changed");
    let calls = fx.calls();
    assert_eq!(calls.iter().filter(|c| c[0] == "create").count(), 1);
    assert!(calls
        .iter()
        .any(|c| c.iter().any(|s| s == "echo old-command")));
    assert!(!calls
        .iter()
        .any(|c| c.iter().any(|s| s == "echo new-command")));
    assert_eq!(
        std::fs::read(format!("{payload}/snapshot.json")).unwrap(),
        snapshot
    );
    fx.fx.write_config("devcontainer.json", "{broken");
    let output = fx.output(&["exec", "--keep", "true"], Some(&version));
    assert_success(&output);
    assert_stderr_contains(&output, "comparison is unavailable");
    std::fs::remove_file(fx.fx.dir.path().join(".devcontainer/devcontainer.json")).unwrap();
    assert_success(&fx.output(&["exec", "--keep", "true"], Some(&version)));
}

#[test]
fn running_runtime_blocks_build_refresh_and_reseed_before_mutation() {
    let fx = FakeDockerFixture::new(root_image_config());
    let version = compatible_patch_version();
    assert_success(&fx.output(&["start"], Some(&version)));
    for args in [
        vec!["build"],
        vec!["build", "--refresh-only"],
        vec!["build", "--reseed-state"],
    ] {
        let output = fx.output(&args, Some(&version));
        assert_failure(&output);
        assert_stderr_contains(&output, "stop it before build");
    }
    assert!(build_calls(&fx.calls()).is_empty());
}

const FORWARDED: &str = r#"{"image":"debian","containerUser":"root","forwardPorts":[4173]}"#;
const IPV4_BINDING: &str = r#"{"20000/tcp":[{"HostIp":"127.0.0.1","HostPort":"4173"}]}"#;
#[test]
fn ipv6_retry_requires_proof_that_startup_never_ran() {
    for failure in ["safe", "uncertain"] {
        let fx = FakeDockerFixture::new(FORWARDED);
        let output = fx
            .dcc(&["start"])
            .env("DCC_FAKE_VERSION_LABEL", compatible_patch_version())
            .env("DCC_FAKE_BINDINGS", IPV4_BINDING)
            .env("DCC_FAKE_START_FAIL", failure)
            .output()
            .unwrap();
        let calls = fx.calls();
        let creates: Vec<_> = calls.iter().filter(|c| c[0] == "create").collect();
        assert!(contains_pair(
            creates[0],
            "--publish",
            "[::1]:4173:20000/tcp"
        ));
        if failure == "safe" {
            assert_success(&output);
            assert_eq!(creates.len(), 2);
            assert!(contains_pair(
                creates[1],
                "--publish",
                "127.0.0.1:4173:20000/tcp"
            ));
            assert!(!creates[1].iter().any(|s| s.contains("[::1]")));
            assert_eq!(
                calls
                    .iter()
                    .filter(|c| c.iter().any(|s| s == "wait-ready"))
                    .count(),
                1
            );
        } else {
            assert_failure(&output);
            assert_eq!(creates.len(), 1);
            assert!(!calls.iter().any(|c| c[0] == "rm"));
        }
    }
}

#[test]
fn forwarding_rejects_old_engines_before_creation_and_unsafe_actual_bindings_after_start() {
    let fx = FakeDockerFixture::new(FORWARDED);
    let output = fx
        .dcc(&["start"])
        .env("DCC_FAKE_VERSION_LABEL", compatible_patch_version())
        .env("DCC_FAKE_ENGINE_VERSION", "27.5.1")
        .output()
        .unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "Engine 28+");
    assert!(!fx.calls().iter().any(|c| c[0] == "create"));
    let output = fx
        .dcc(&["start"])
        .env("DCC_FAKE_VERSION_LABEL", compatible_patch_version())
        .env(
            "DCC_FAKE_BINDINGS",
            IPV4_BINDING.replace("127.0.0.1", "0.0.0.0"),
        )
        .output()
        .unwrap();
    assert_failure(&output);
    assert_stderr_contains(&output, "unexpected forwarding publication");
    assert!(fx.calls().iter().any(|c| c[0] == "stop"));
}

#[test]
fn degraded_forwarding_allows_frozen_commands_and_does_not_recreate() {
    let fx = FakeDockerFixture::new(root_image_config());
    let version = compatible_patch_version();
    assert_success(&fx.output(&["start"], Some(&version)));
    let output = fx
        .dcc(&["exec", "--keep", "true"])
        .env("DCC_FAKE_VERSION_LABEL", version)
        .env("DCC_FAKE_RELAY_CODE", "1")
        .output()
        .unwrap();
    assert_success(&output);
    assert_stderr_contains(&output, "forwarding is degraded");
    assert_eq!(fx.calls().iter().filter(|c| c[0] == "create").count(), 1);
}

#[test]
fn fingerprint_distinguishes_absent_empty_and_changed_local_environment() {
    let fx = FakeDockerFixture::new(
        r#"{"image":"debian","containerUser":"root","remoteEnv":{"VALUE":"${localEnv:DCC_TEST_FROZEN_ENV:default}"}}"#,
    );
    let version = compatible_patch_version();
    assert_success(
        &fx.dcc(&["start"])
            .env("DCC_FAKE_VERSION_LABEL", &version)
            .env_remove("DCC_TEST_FROZEN_ENV")
            .output()
            .unwrap(),
    );
    for value in ["", "changed"] {
        let output = fx
            .dcc(&[
                "exec",
                "--keep",
                "echo",
                "${localEnv:DCC_TEST_FROZEN_ENV:default}",
            ])
            .env("DCC_FAKE_VERSION_LABEL", &version)
            .env("DCC_TEST_FROZEN_ENV", value)
            .output()
            .unwrap();
        assert_success(&output);
        assert_stderr_contains(&output, "configuration changed");
        assert!(fx.calls().last().unwrap().iter().any(|arg| arg == value));
    }
}

#[test]
fn explicit_resource_changes_warn_but_omitted_defaults_do_not() {
    let fx = FakeDockerFixture::new(root_image_config());
    let version = compatible_patch_version();
    assert_success(&fx.output(&["start", "--memory", "8g"], Some(&version)));
    let output = fx.output(&["start"], Some(&version));
    assert_success(&output);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("configuration changed"));
    let output = fx.output(&["start", "--memory", "4g"], Some(&version));
    assert_success(&output);
    assert_stderr_contains(&output, "configuration changed");
}

#[test]
fn immutable_assets_prune_only_released_unreferenced_instances() {
    let fx = FakeDockerFixture::new(root_image_config());
    let version = compatible_patch_version();
    assert_success(&fx.output(&["start"], Some(&version)));
    let original =
        PathBuf::from(std::fs::read_to_string(fx.state.with_extension("payload")).unwrap());
    assert!(original.parent().unwrap().join("released").exists());
    use std::os::unix::fs::PermissionsExt as _;
    assert_eq!(
        std::fs::metadata(original.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(original.join("snapshot.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    // A stopped container still references the original payload.
    std::fs::remove_file(&fx.state).unwrap();
    assert_success(&fx.output(&["start"], Some(&version)));
    assert!(original.exists());
    // Uncertain inspection retains even the now-unreferenced original.
    std::fs::remove_file(&fx.state).unwrap();
    let output = fx
        .dcc(&["start"])
        .env("DCC_FAKE_VERSION_LABEL", &version)
        .env("DCC_FAKE_MOUNT_INSPECT_FAIL", "1")
        .output()
        .unwrap();
    assert_success(&output);
    assert_stderr_contains(&output, "retaining assets");
    assert!(original.exists());
    std::fs::remove_file(&fx.state).unwrap();
    std::fs::remove_file(fx.state.with_extension("inspect")).unwrap();
    // An in-progress launch has no release marker and must survive an empty Docker list.
    let pending = original
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("abc-def-123");
    std::fs::create_dir(&pending).unwrap();
    assert_success(&fx.output(&["start"], Some(&version)));
    assert!(!original.exists());
    assert!(pending.exists());
}
