//! Deterministic real-Docker persistence regressions. No upstream tool downloads.
//! Run explicitly with `cargo test --test persistence -- --ignored --test-threads=1`.
#![cfg(unix)]

mod common;
use common::*;

use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt as _},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

const ROOT: &str = "/home/fixture/.persistence-fixture";

struct PersistenceFixture {
    fx: Fixture,
    image: String,
}

impl PersistenceFixture {
    fn new() -> Self {
        let fx = Fixture::new();
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/persistence");
        copy_tree(&source, &fx.dir.path().join(".devcontainer"));
        let output = fx.dcc(&["id"]).output().unwrap();
        assert_success(&output);
        let image = String::from_utf8(output.stdout).unwrap().trim().to_owned();
        let fixture = Self { fx, image };
        fixture.config("v1", true);
        fixture
    }

    fn config(&self, version: &str, hooks: bool) {
        let mut config = serde_json::json!({
            "build": {"dockerfile": "Dockerfile"},
            "containerUser": "fixture",
            "features": {"./feature": {"imageVersion": version}}
        });
        if hooks {
            config["onCreateCommand"] = serde_json::json!(
                "/usr/local/libexec/persistence-fixture-assert-ready > /workspace/build-ready"
            );
            config["postStartCommand"] = serde_json::json!(
                "/usr/local/libexec/persistence-fixture-assert-ready > /workspace/start-ready"
            );
        }
        self.fx.write_config(
            "devcontainer.json",
            &serde_json::to_string(&config).unwrap(),
        );
    }

    fn dcc(&self, args: &[&str]) -> Output {
        self.fx.dcc(args).output().unwrap()
    }

    fn exec(&self, script: &str) -> Output {
        self.dcc(&["exec", "/bin/sh", "-ec", script])
    }

    fn state(&self) -> PathBuf {
        self.fx
            .dir
            .path()
            .join(".dcc/devcontainer/state/home/fixture/.persistence-fixture")
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.fx.dir.path().join(path)).unwrap()
    }
}

impl Drop for PersistenceFixture {
    fn drop(&mut self) {
        // The fixture has a unique workspace identity. Never touch another
        // profile, Docker volumes, or a user's persistent cache.
        let output = Command::new("docker")
            .args([
                "ps",
                "-aq",
                "--filter",
                &format!("label=dcc.container_id={}", self.image),
            ])
            .output();
        if let Ok(output) = output {
            for id in String::from_utf8_lossy(&output.stdout).lines() {
                let _ = Command::new("docker").args(["rm", "-f", id]).output();
            }
        }
        for image in [&self.image, &format!("{}-base", self.image)] {
            let _ = Command::new("docker")
                .args(["image", "rm", "-f", image])
                .output();
        }
        // Tests temporarily make a nested directory unreadable to detect scans.
        let inaccessible = self.state().join("cache/unreadable");
        let _ = fs::set_permissions(inaccessible, fs::Permissions::from_mode(0o700));
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn recovered_entries(root: &Path, original: &str) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".dcc-exclusion-backup.")
        })
        .filter(|path| {
            fs::read_to_string(path.join("original-path"))
                .unwrap()
                .trim()
                == format!("{ROOT}/{original}")
        })
        .map(|path| path.join("original"))
        .collect()
}

#[test]
#[ignore]
fn exclusions_are_ready_before_hooks_and_reset_on_replacement() {
    let fx = PersistenceFixture::new();
    assert_success(&fx.dcc(&["build"]));
    assert_eq!(fx.read("build-ready"), "ready\n");
    assert_success(&fx.exec(&format!(
        "test \"$({ROOT}/packages/bin/fixture-tool)\" = v1; \
         test -z \"$(ls -A {ROOT}/scratch)\"; \
         printf credential > {ROOT}/auth.json; \
         printf credential > {ROOT}/auth.json.tmp; mv {ROOT}/auth.json.tmp {ROOT}/auth.json; \
         printf history > {ROOT}/sessions/user; \
         printf cached > {ROOT}/cache/user; \
         printf runtime > {ROOT}/packages/release; \
         printf temporary > {ROOT}/scratch/temporary"
    )));
    assert_eq!(fx.read("start-ready"), "ready\n");
    assert_success(&fx.exec(&format!(
        "test \"$(cat {ROOT}/packages/release)\" = v1; \
         test ! -e {ROOT}/scratch/temporary; \
         test \"$(cat {ROOT}/auth.json)\" = credential; \
         test \"$(cat {ROOT}/sessions/user)\" = history; \
         test \"$(cat {ROOT}/cache/user)\" = cached"
    )));

    fx.config("v2", true);
    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.exec(&format!(
        "test \"$({ROOT}/packages/bin/fixture-tool)\" = v2; \
         test \"$(cat {ROOT}/preferences/settings)\" = image-default-v1; \
         test \"$(cat {ROOT}/auth.json)\" = credential; \
         test \"$(cat {ROOT}/sessions/user)\" = history; \
         test \"$(cat {ROOT}/cache/user)\" = cached; \
         test -z \"$(ls -A {ROOT}/scratch)\""
    )));
}

#[test]
#[ignore]
fn legacy_exclusions_and_stale_links_are_preserved_for_recovery() {
    let fx = PersistenceFixture::new();
    let state = fx.state();
    fs::create_dir_all(state.join("packages/deep")).unwrap();
    fs::create_dir_all(state.join("preferences")).unwrap();
    fs::write(state.join("packages/deep/old-install"), "legacy-software").unwrap();
    fs::write(state.join("preferences/settings"), "existing-preferences").unwrap();
    fs::write(state.join("auth.json"), "existing-credential").unwrap();
    symlink("/missing/old-scratch", state.join("scratch")).unwrap();
    let ledger = fx.fx.dir.path().join(".dcc/devcontainer.seed.json");
    // Ignoring obsolete ledgers must not require parsing, rewriting, or deleting them.
    fs::write(&ledger, "old-ledger-preserved-verbatim").unwrap();

    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.exec(&format!(
        "test \"$({ROOT}/packages/bin/fixture-tool)\" = v1; \
         test \"$(cat {ROOT}/preferences/settings)\" = existing-preferences; \
         test \"$(cat {ROOT}/auth.json)\" = existing-credential; \
         test ! -e {ROOT}/sessions/initial"
    )));
    assert_eq!(
        fs::read_to_string(&ledger).unwrap(),
        "old-ledger-preserved-verbatim"
    );
    let packages = recovered_entries(&state, "packages");
    assert_eq!(
        packages.len(),
        1,
        "legacy package tree must be retained exactly once"
    );
    assert_eq!(
        fs::read_to_string(packages[0].join("deep/old-install")).unwrap(),
        "legacy-software"
    );
    let scratch = recovered_entries(&state, "scratch");
    assert_eq!(scratch.len(), 1, "stale link must be retained exactly once");
    assert_eq!(
        fs::read_link(&scratch[0]).unwrap(),
        Path::new("/missing/old-scratch")
    );
    assert_success(&fx.exec("true"));
    assert_eq!(recovered_entries(&state, "packages"), packages);
    assert_eq!(recovered_entries(&state, "scratch"), scratch);

    // A stale link can resolve successfully and still select the wrong tree.
    // Preserve that link too, without modifying the user directory it selected.
    fs::remove_file(state.join("scratch")).unwrap();
    symlink(format!("{ROOT}/preferences"), state.join("scratch")).unwrap();
    assert_success(&fx.exec("true"));
    let recovered_scratch = recovered_entries(&state, "scratch");
    assert_eq!(recovered_scratch.len(), 2);
    assert!(recovered_scratch
        .iter()
        .any(|path| { fs::read_link(path).unwrap() == Path::new(&format!("{ROOT}/preferences")) }));
    assert_eq!(
        fs::read_to_string(state.join("preferences/settings")).unwrap(),
        "existing-preferences"
    );
}

#[test]
#[ignore]
fn existing_empty_root_is_not_reinitialized() {
    let fx = PersistenceFixture::new();
    fs::create_dir_all(fx.state()).unwrap();
    fx.config("v1", false);
    assert_success(&fx.dcc(&["build"]));
    assert_success(&fx.exec(&format!(
        "test -L {ROOT}/packages; test -L {ROOT}/scratch; \
         test ! -e {ROOT}/preferences/settings; test ! -e {ROOT}/sessions/initial"
    )));
}

#[test]
#[ignore]
fn concurrent_attachment_preserves_migration_once() {
    let fx = PersistenceFixture::new();
    assert_success(&fx.dcc(&["build"]));
    // Force migration on first runtime startup, where independent processes
    // contend for both persistence preparation and container creation.
    fs::remove_file(fx.state().join("packages")).unwrap();
    fs::create_dir(fx.state().join("packages")).unwrap();
    fs::write(fx.state().join("packages/user-file"), "recover-me").unwrap();
    fs::write(fx.state().join("preferences/settings"), "keep-me").unwrap();
    let mut commands = Vec::new();
    for _ in 0..6 {
        commands.push(
            fx.fx
                .dcc(&[
                    "exec",
                    "--keep",
                    "/usr/local/libexec/persistence-fixture-assert-ready",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    for child in commands {
        assert_success(&child.wait_with_output().unwrap());
    }
    let recovered = recovered_entries(&fx.state(), "packages");
    assert_eq!(recovered.len(), 1);
    assert_eq!(
        fs::read_to_string(recovered[0].join("user-file")).unwrap(),
        "recover-me"
    );
    assert_success(&fx.dcc(&["stop"]));
}

#[test]
#[ignore]
fn rebuilding_large_state_never_reads_durable_contents() {
    let fx = PersistenceFixture::new();
    assert_success(&fx.dcc(&["build"]));
    let cache = fx.state().join("cache");
    let large = fs::File::create(cache.join("large-sparse-cache")).unwrap();
    large.set_len(8 * 1024 * 1024 * 1024).unwrap();
    fs::create_dir(cache.join("unreadable")).unwrap();
    fs::write(cache.join("unreadable/credential"), "never-read-me").unwrap();
    fs::set_permissions(cache.join("unreadable"), fs::Permissions::from_mode(0o0)).unwrap();
    // A scan that attempts to open every file blocks on this FIFO. The deadline
    // prevents an accidental implementation regression from hanging CI forever.
    assert!(Command::new("mkfifo")
        .arg(cache.join("do-not-read-fifo"))
        .status()
        .unwrap()
        .success());
    let started = Instant::now();
    let mut child = fx
        .fx
        .dcc(&["build"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut peak_rss_kib = 0;
    loop {
        // Linux /proc reports the dcc process itself, excluding Docker's daemon
        // and compiler memory. On Desktop the behavioral no-scan checks still
        // run, while memory measurement requires a platform-specific method.
        if let Ok(status) = fs::read_to_string(format!("/proc/{}/status", child.id())) {
            if let Some(rss) = status.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
            }) {
                peak_rss_kib = peak_rss_kib.max(rss);
            }
        }
        if child.try_wait().unwrap().is_some() {
            assert_success(&child.wait_with_output().unwrap());
            break;
        }
        if started.elapsed() > Duration::from_secs(90) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("rebuild scanned durable state or exceeded its 90s deadline");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    if cfg!(target_os = "linux") {
        assert!(peak_rss_kib > 0, "dcc RSS measurement was unavailable");
        eprintln!(
            "8 GiB durable-state rebuild: peak dcc RSS {peak_rss_kib} KiB, elapsed {:?}",
            started.elapsed()
        );
        assert!(
            peak_rss_kib < 128 * 1024,
            "dcc RSS grew with durable state: {peak_rss_kib} KiB"
        );
    }
    assert_eq!(
        fs::metadata(cache.join("large-sparse-cache"))
            .unwrap()
            .len(),
        8 * 1024 * 1024 * 1024
    );
    fs::set_permissions(cache.join("unreadable"), fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
#[ignore]
fn symlinked_state_root_is_refused_without_touching_target() {
    let fx = PersistenceFixture::new();
    let outside = fx.fx.dir.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("credential"), "preserve-me").unwrap();
    fs::create_dir_all(fx.state().parent().unwrap()).unwrap();
    symlink(&outside, fx.state()).unwrap();
    assert_failure(&fx.dcc(&["build"]));
    assert_eq!(
        fs::read_to_string(outside.join("credential")).unwrap(),
        "preserve-me"
    );
    assert_eq!(fs::read_dir(outside).unwrap().count(), 1);
}

#[test]
#[ignore]
fn symlinked_cache_ancestor_is_refused_before_state_initialization() {
    let fx = PersistenceFixture::new();
    let outside = fx.fx.dir.path().join("outside-cache");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("credential"), "preserve-me").unwrap();
    symlink(&outside, fx.fx.dir.path().join(".dcc")).unwrap();
    assert_failure(&fx.dcc(&["build"]));
    assert_eq!(
        fs::read_to_string(outside.join("credential")).unwrap(),
        "preserve-me"
    );
    assert!(
        !outside.join("devcontainer/state").exists(),
        "symlinked cache must not receive image contents"
    );
}

#[test]
#[ignore]
fn unwritable_image_target_fails_before_user_hooks() {
    let fx = PersistenceFixture::new();
    let installer = fx.fx.dir.path().join(".devcontainer/feature/install.sh");
    let mut script = fs::read_to_string(&installer).unwrap();
    script.push_str("\nchmod 0500 /opt/dcc-local/persistence/packages\n");
    fs::write(installer, script).unwrap();
    let output = fx.dcc(&["build"]);
    assert_failure(&output);
    assert_stderr_contains(
        &output,
        "excluded target must be a writable image directory",
    );
    assert!(!fx.fx.dir.path().join("build-ready").exists());
    assert_eq!(
        fs::read_to_string(fx.state().join("preferences/settings")).unwrap(),
        "image-default-v1\n"
    );
}

#[test]
#[ignore]
fn explicit_mount_cannot_mask_excluded_image_target() {
    let fx = PersistenceFixture::new();
    fx.config("v1", false);
    let override_root = fx.fx.dir.path().join("override");
    fs::create_dir(&override_root).unwrap();
    let config_path = fx.fx.dir.path().join(".devcontainer/devcontainer.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    config["mounts"] = serde_json::json!([format!(
        "type=bind,source={},target=/opt/dcc-local/persistence",
        override_root.display()
    )]);
    fs::write(config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert_failure(&fx.dcc(&["build"]));
    assert!(
        !fx.state().exists(),
        "masked target must fail before durable initialization"
    );
    assert_eq!(fs::read_dir(override_root).unwrap().count(), 0);
}

#[test]
#[ignore]
fn workspace_cannot_be_an_excluded_image_target() {
    let fx = PersistenceFixture::new();
    fx.config("v1", false);
    let feature = fx.fx.dir.path().join(".devcontainer/feature");
    let metadata = feature.join("devcontainer-feature.json");
    let contents = fs::read_to_string(&metadata).unwrap().replace(
        "/opt/dcc-local/persistence/packages",
        "/workspace/image-packages",
    );
    fs::write(metadata, contents).unwrap();
    let installer = feature.join("install.sh");
    let contents = fs::read_to_string(&installer)
        .unwrap()
        .replace(
            "mkdir -p /opt/dcc-local/persistence",
            "mkdir -p /workspace /opt/dcc-local/persistence",
        )
        .replace(
            "/opt/dcc-local/persistence/packages",
            "/workspace/image-packages",
        );
    fs::write(installer, contents).unwrap();
    assert_failure(&fx.dcc(&["build"]));
    assert!(
        !fx.state().exists(),
        "workspace target must fail before durable initialization"
    );
    assert!(!fx.fx.dir.path().join("image-packages").exists());
}
