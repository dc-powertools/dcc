#![allow(dead_code)]

use std::path::PathBuf;

pub struct Fixture {
    pub dir: tempfile::TempDir,
}

impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        std::fs::create_dir(dir.path().join(".devcontainer"))
            .expect("failed to create .devcontainer dir");
        Self { dir }
    }

    /// Write a file inside .devcontainer/ with the given name and content.
    pub fn write_config(&self, name: &str, content: &str) -> PathBuf {
        let path = self.dir.path().join(".devcontainer").join(name);
        std::fs::write(&path, content).expect("failed to write config file");
        path
    }

    /// Returns a Command for `dcc` with given args, cwd set to the fixture root.
    pub fn dcc(&self, args: &[&str]) -> std::process::Command {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_dcc"));
        cmd.args(args).current_dir(self.dir.path());
        cmd
    }
}

pub fn assert_failure(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "expected command to fail but it succeeded\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

pub fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected command to succeed but it failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

pub fn assert_stderr_contains(output: &std::process::Output, needle: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(needle),
        "expected stderr to contain {:?}\nactual stderr: {}",
        needle,
        stderr,
    );
}
