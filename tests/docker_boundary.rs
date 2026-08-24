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
if [ "$command_name" = build ]; then
    cat >/dev/null
    exit 0
fi

if [ "$command_name" = image ] && [ "${2-}" = inspect ]; then
    if [ "${3-}" != --format ]; then
        exit 0
    fi
    case "${4-}" in
        *dcc.version*) printf '%s\n' "${DCC_FAKE_VERSION_LABEL-<no value>}" ;;
        *devcontainer.metadata*) printf '%s\n' '<no value>' ;;
        *Config.Env*) printf '%s\n' '[]' ;;
        *dcc.seed*) printf '%s\n' '<no value>' ;;
        *) printf '%s\n' '<no value>' ;;
    esac
    exit 0
fi

if [ "$command_name" = ps ]; then
    if [ -s "$DCC_FAKE_DOCKER_STATE" ]; then
        cat "$DCC_FAKE_DOCKER_STATE"
        printf '\n'
    fi
    exit 0
fi

if [ "$command_name" = inspect ]; then
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
    for argument in "$@"; do
        case "$argument" in
            stop|stop-now) rm -f "$DCC_FAKE_DOCKER_STATE" ;;
        esac
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
        Self {
            fx,
            path,
            log,
            state,
        }
    }

    fn dcc(&self, args: &[&str]) -> Command {
        let mut command = self.fx.dcc(args);
        command
            .env("PATH", &self.path)
            .env("DCC_FAKE_DOCKER_LOG", &self.log)
            .env("DCC_FAKE_DOCKER_STATE", &self.state);
        command
    }

    fn output(&self, args: &[&str], version: Option<&str>) -> Output {
        let mut command = self.dcc(args);
        if let Some(version) = version {
            command.env("DCC_FAKE_VERSION_LABEL", version);
        }
        command.output().unwrap()
    }

    fn set_running(&self, name: &str) {
        std::fs::write(&self.state, name).unwrap();
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
        .find(|call| call.first().is_some_and(|arg| arg == "run"))
        .expect("expected a docker run call")
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
