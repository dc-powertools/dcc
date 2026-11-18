//! Linux behavioral checks for generated lifecycle scripts. Test-only checkpoints
//! control the interleaving; no production delay is used as a correctness barrier.
use super::*;
use std::{
    fs,
    os::unix::process::CommandExt as _,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(10);

struct Process(Child);

impl Process {
    fn wait(&mut self) -> ExitStatus {
        let until = Instant::now() + DEADLINE;
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < until, "child {} did not exit", self.0.id());
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn signal(&self, signal: &str) {
        assert!(Command::new("kill")
            .args([signal, "--", &self.0.id().to_string()])
            .status()
            .unwrap()
            .success());
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Also collect descendants created by shell checkpoint loops or commands.
        let _ = Command::new("kill")
            .args(["-KILL", "--", &format!("-{}", self.0.id())])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.0.wait();
    }
}

struct Scripts {
    root: tempfile::TempDir,
    state: PathBuf,
    share: PathBuf,
}

impl Scripts {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let state = root.path().join("state");
        let share = root.path().join("share");
        fs::create_dir_all(state.join("active")).unwrap();
        fs::create_dir_all(state.join("waiters")).unwrap();
        fs::create_dir_all(share.join("rt")).unwrap();
        fs::write(state.join("lifecycle.lock"), "").unwrap();
        for (name, content, _) in baked_supervisor_assets() {
            let source = String::from_utf8(content)
                .unwrap()
                .replace(DCC_SHARE, share.to_str().unwrap())
                .replace(STATE_DIR, state.to_str().unwrap())
                // Some scenarios publish completed work before starting PID 1.
                // Reuse the fixture inode; never replace the lock itself.
                .replace(
                    "(umask 022; set -C; : > \"$LOCK\")",
                    "[ -f \"$LOCK\" ] || (umask 022; set -C; : > \"$LOCK\")",
                );
            write_script(&share, name.rsplit('/').next().unwrap(), source).unwrap();
        }
        Self { root, state, share }
    }

    fn edit(&self, script: &str, before: &str, after: &str) {
        let path = self.share.join(script);
        let source = fs::read_to_string(&path).unwrap();
        assert_eq!(source.matches(before).count(), 1, "checkpoint {before:?}");
        fs::write(path, source.replacen(before, after, 1)).unwrap();
    }

    fn gate(&self, script: &str, anchor: &str, name: &str, after: bool) {
        let gate = format!(
            "\nif [ ! -f \"$STATE/{name}.hit\" ]; then\n : > \"$STATE/{name}.hit\"\n while [ ! -f \"$STATE/{name}.go\" ]; do sleep 0.01; done\nfi\n"
        );
        let replacement = if after {
            format!("{anchor}{gate}")
        } else {
            format!("{gate}{anchor}")
        };
        self.edit(script, anchor, &replacement);
    }

    fn spawn(&self, script: &str, args: &[&str]) -> Process {
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.path().join(format!("{script}.log")))
            .unwrap();
        Process(
            Command::new(self.share.join(script))
                .args(args)
                .env("TEST_STATE", &self.state)
                .process_group(0)
                .stdin(Stdio::null())
                .stdout(log.try_clone().unwrap())
                .stderr(log)
                .spawn()
                .unwrap(),
        )
    }

    fn wait_file(&self, name: &str) {
        let until = Instant::now() + DEADLINE;
        while !self.state.join(name).exists() {
            assert!(
                Instant::now() < until,
                "missing checkpoint {name}; logs: {}",
                self.logs()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn logs(&self) -> String {
        ["dcc-supervisor", "dcc-exec", "dcc-ctl"]
            .iter()
            .map(|name| {
                fs::read_to_string(self.root.path().join(format!("{name}.log"))).unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn release(&self, name: &str) {
        fs::write(self.state.join(format!("{name}.go")), "").unwrap();
    }

    fn ready(&self) {
        fs::write(self.state.join("bootstrap-status"), "0").unwrap();
    }

    fn records(&self) -> Vec<PathBuf> {
        fs::read_dir(self.state.join("active"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect()
    }

    fn running_command(&self) -> Process {
        self.spawn(
            "dcc-exec",
            &[
                "sh",
                "-c",
                ": > \"$TEST_STATE/command.hit\"; while [ ! -f \"$TEST_STATE/command.go\" ]; do sleep 0.01; done",
            ],
        )
    }
}

#[test]
fn lifecycle_fast_command_leaves_evidence_until_supervisor_retires_it() {
    let scripts = Scripts::new();
    scripts.ready();
    let mut command = scripts.spawn("dcc-exec", &["sh", "-c", "exit 17"]);
    assert_eq!(command.wait().code(), Some(17), "{}", scripts.logs());
    assert_eq!(scripts.records().len(), 1, "wrapper erased registration");
    assert!(!scripts.state.join("arrived").exists());
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    assert!(supervisor.wait().success(), "{}", scripts.logs());
    assert!(scripts.records().is_empty());
    assert!(scripts.state.join("stopping").exists());
}

#[test]
fn lifecycle_registration_wins_before_initial_empty_observation() {
    for orphan_expired in [false, true] {
        let scripts = Scripts::new();
        scripts.ready();
        if orphan_expired {
            scripts.edit("dcc-supervisor", "-ge \"10\"", "-ge \"0\"");
        }
        scripts.gate("dcc-supervisor", "while true; do\n", "poll", true);
        scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
        let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
        scripts.wait_file("poll.hit");
        let mut command = scripts.running_command();
        scripts.wait_file("command.hit");
        scripts.release("poll");
        scripts.wait_file("scan.hit");
        assert!(!scripts.state.join("stopping").exists());
        assert_eq!(scripts.records().len(), 1);
        scripts.release("scan");
        scripts.release("command");
        assert!(command.wait().success());
        assert!(supervisor.wait().success(), "{}", scripts.logs());
    }
}

fn drain_wins(orphan: bool) {
    let scripts = Scripts::new();
    scripts.ready();
    if orphan {
        scripts.edit("dcc-supervisor", "-ge \"10\"", "-ge \"0\"");
    } else {
        let mut previous = scripts.spawn("dcc-exec", &["true"]);
        assert!(previous.wait().success());
        assert_eq!(scripts.records().len(), 1);
    }
    scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
    scripts.gate("dcc-exec", "lifecycle_lock\n", "admit", false);
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    scripts.wait_file("scan.hit");
    let mut command = scripts.running_command();
    scripts.wait_file("admit.hit");
    scripts.release("admit");
    // The supervisor still owns the lock, so admission cannot run user code.
    assert!(!scripts.state.join("command.hit").exists());
    scripts.release("scan");
    assert!(supervisor.wait().success(), "{}", scripts.logs());
    assert_eq!(command.wait().code(), Some(253), "{}", scripts.logs());
    assert!(!scripts.state.join("command.hit").exists());
}

#[test]
fn lifecycle_committed_drain_rejects_registration_after_last_retirement() {
    drain_wins(false);
}

#[test]
fn lifecycle_orphan_expiry_and_registration_share_the_lock() {
    drain_wins(true);
}

#[test]
fn lifecycle_new_registration_wins_before_last_record_retirement() {
    let scripts = Scripts::new();
    scripts.ready();
    let mut previous = scripts.spawn("dcc-exec", &["true"]);
    assert!(previous.wait().success());
    scripts.gate("dcc-supervisor", "while true; do\n", "poll", true);
    scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    scripts.wait_file("poll.hit");
    let mut next = scripts.running_command();
    scripts.wait_file("command.hit");
    assert_eq!(scripts.records().len(), 2);
    scripts.release("poll");
    scripts.wait_file("scan.hit");
    assert_eq!(scripts.records().len(), 1);
    assert!(!scripts.state.join("stopping").exists());
    scripts.release("scan");
    scripts.release("command");
    assert!(next.wait().success());
    assert!(supervisor.wait().success(), "{}", scripts.logs());
}

#[test]
fn lifecycle_killed_wrapper_record_is_reclaimed_without_cli_acknowledgment() {
    let scripts = Scripts::new();
    scripts.ready();
    let mut command = scripts.running_command();
    scripts.wait_file("command.hit");
    command.signal("-KILL");
    assert!(!command.wait().success());
    assert_eq!(scripts.records().len(), 1);
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    assert!(supervisor.wait().success(), "{}", scripts.logs());
    assert!(scripts.records().is_empty());
    scripts.release("command");
}

#[test]
fn lifecycle_lock_is_not_inherited_by_user_children_or_readiness_waits() {
    let scripts = Scripts::new();
    scripts.ready();
    let mut command = scripts.spawn(
        "dcc-exec",
        &[
            "sh",
            "-c",
            "[ ! -e /proc/self/fd/9 ] && flock -n \"$TEST_STATE/lifecycle.lock\" sh -c ': > \"$TEST_STATE/child-lock.hit\"'",
        ],
    );
    assert!(command.wait().success(), "{}", scripts.logs());
    scripts.wait_file("child-lock.hit");
    fs::remove_file(scripts.state.join("bootstrap-status")).unwrap();
    let mut waiting = scripts.spawn("dcc-exec", &["true"]);
    let until = Instant::now() + DEADLINE;
    loop {
        if fs::read_dir(scripts.state.join("waiters"))
            .unwrap()
            .next()
            .is_some()
        {
            break;
        }
        assert!(Instant::now() < until, "wrapper never waited for readiness");
        thread::sleep(Duration::from_millis(10));
    }
    let mut stop = scripts.spawn("dcc-ctl", &["stop"]);
    assert!(
        stop.wait().success(),
        "control blocked on readiness-held lock"
    );
    waiting.signal("-KILL");
    waiting.wait();
}

#[test]
fn lifecycle_sigkill_releases_an_acquired_admission_lock() {
    let scripts = Scripts::new();
    scripts.ready();
    scripts.gate("dcc-exec", "lifecycle_lock\n", "locked", true);
    let mut holder = scripts.spawn("dcc-exec", &["true"]);
    scripts.wait_file("locked.hit");
    holder.signal("-KILL");
    holder.wait();
    let mut stop = scripts.spawn("dcc-ctl", &["stop"]);
    assert!(stop.wait().success(), "{}", scripts.logs());
    assert!(scripts.state.join("stopping").exists());
}

#[test]
fn lifecycle_durable_promotion_wins_before_poll_and_stop_closes_admission() {
    let scripts = Scripts::new();
    scripts.ready();
    let mut previous = scripts.spawn("dcc-exec", &["true"]);
    assert!(previous.wait().success());
    scripts.gate("dcc-supervisor", "while true; do\n", "poll", true);
    scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    scripts.wait_file("poll.hit");
    let mut promote = scripts.spawn("dcc-ctl", &["mode", "durable"]);
    assert!(promote.wait().success());
    scripts.release("poll");
    scripts.wait_file("scan.hit");
    scripts.release("scan");
    let mut command = scripts.running_command();
    scripts.wait_file("command.hit");
    let mut stop = scripts.spawn("dcc-ctl", &["stop"]);
    assert!(stop.wait().success());
    let mut refused = scripts.spawn("dcc-exec", &["true"]);
    assert_eq!(refused.wait().code(), Some(253));
    assert!(supervisor.0.try_wait().unwrap().is_none());
    let mut late_promotion = scripts.spawn("dcc-ctl", &["mode", "durable"]);
    assert_eq!(late_promotion.wait().code(), Some(253));
    scripts.release("command");
    assert!(command.wait().success());
    assert!(supervisor.wait().success(), "{}", scripts.logs());
}

fn stat(pid: u32, comm: &str, state: &str, start: u64) -> String {
    format!(
        "{pid} ({comm}) {state} {} {start} 0 0\n",
        ["0"; 18].join(" ")
    )
}

fn helper(scripts: &Scripts, proc_root: Option<&Path>, body: &str) -> std::process::Output {
    let functions = if let Some(root) = proc_root {
        lifecycle_functions().replace("/proc", root.to_str().unwrap())
    } else {
        lifecycle_functions().to_string()
    };
    Command::new("sh")
        .arg("-c")
        .arg(format!(
            "set -eu\nSTATE={}\nACTIVE=\"$STATE/active\"\nSTOPPING=\"$STATE/stopping\"\narrived=0\nunknown_reported=0\n{}\n{}\n",
            sh_quote(scripts.state.to_str().unwrap()),
            functions,
            body
        ))
        .output()
        .unwrap()
}

#[test]
fn lifecycle_proc_stat_parses_parenthesized_comm_and_rejects_unknown_data() {
    let scripts = Scripts::new();
    let proc_root = scripts.root.path().join("proc");
    fs::create_dir_all(proc_root.join("123")).unwrap();
    for (comm, state, start) in [
        ("sh", "S", 42),
        ("space and ) close ( hi)", "T", 57),
        ("multi\nline ) name", "Z", 64),
    ] {
        fs::write(proc_root.join("123/stat"), stat(123, comm, state, start)).unwrap();
        let output = helper(
            &scripts,
            Some(&proc_root),
            "proc_identity 123; printf '%s %s' \"$proc_state\" \"$proc_start\"",
        );
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{state} {start}")
        );
    }
    for invalid in [
        "123 (comm) S 0 0\n".to_string(),
        stat(123, "comm", "?", 42),
        stat(456, "wrong pid", "S", 42),
        stat(123, "comm", "S", 42).replacen("42", "not-a-start-time", 1),
    ] {
        fs::write(proc_root.join("123/stat"), invalid).unwrap();
        let output = helper(
            &scripts,
            Some(&proc_root),
            "if proc_identity 123; then exit 1; else [ \"$?\" -eq 2 ]; fi",
        );
        assert!(output.status.success(), "malformed /proc data accepted");
    }
    fs::remove_file(proc_root.join("123/stat")).unwrap();
    let output = helper(
        &scripts,
        Some(&proc_root),
        "if proc_identity 123; then exit 1; else [ \"$?\" -eq 2 ]; fi\nif proc_identity 999; then exit 1; else [ \"$?\" -eq 1 ]; fi",
    );
    assert!(output.status.success(), "unknown read treated as absent");
}

#[test]
fn lifecycle_record_identity_classifies_reuse_zombies_stops_and_unknowns() {
    let scripts = Scripts::new();
    let proc_root = scripts.root.path().join("proc");
    fs::create_dir_all(proc_root.join("123")).unwrap();
    let record = scripts.state.join("active/test");
    for (record_data, proc_data, expected) in [
        ("123 42\n", stat(123, "sh", "S", 42), "live"),
        ("123 42\n", stat(123, "stopped", "T", 42), "live"),
        ("123 42\n", stat(123, "traced", "t", 42), "live"),
        ("123 42\n", stat(123, "reused", "S", 43), "dead"),
        ("123 42\n", stat(123, "zombie", "Z", 42), "dead"),
        ("123 42\n", stat(123, "dead", "X", 42), "dead"),
        ("123 42\n", "unparseable\n".into(), "unknown"),
        ("123\n", stat(123, "sh", "S", 42), "unknown"),
        ("123 42 extra\n", stat(123, "sh", "S", 42), "unknown"),
        ("0 42\n", stat(123, "sh", "S", 42), "unknown"),
        ("123 missing\n", stat(123, "sh", "S", 42), "unknown"),
    ] {
        fs::write(&record, record_data).unwrap();
        fs::write(proc_root.join("123/stat"), proc_data).unwrap();
        let output = helper(
            &scripts,
            Some(&proc_root),
            "inspect_record \"$ACTIVE/test\"; printf '%s' \"$identity_result\"",
        );
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    fs::write(&record, "999 42\n").unwrap();
    let output = helper(
        &scripts,
        Some(&proc_root),
        "inspect_record \"$ACTIVE/test\"; [ \"$identity_result\" = dead ]",
    );
    assert!(output.status.success());
    fs::remove_file(&record).unwrap();
    std::os::unix::fs::symlink(proc_root.join("123/stat"), &record).unwrap();
    let output = helper(
        &scripts,
        Some(&proc_root),
        "inspect_record \"$ACTIVE/test\"; [ \"$identity_result\" = unknown ]",
    );
    assert!(output.status.success());
}

#[test]
fn lifecycle_unknown_records_block_orphan_drain_and_emit_bounded_diagnostics() {
    let scripts = Scripts::new();
    scripts.ready();
    fs::write(scripts.state.join("active/broken"), "not an identity").unwrap();
    scripts.edit("dcc-supervisor", "-ge \"10\"", "-ge \"0\"");
    scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    scripts.wait_file("scan.hit");
    assert!(!scripts.state.join("stopping").exists());
    assert_eq!(scripts.records().len(), 1);
    scripts.release("scan");
    let mut stop = scripts.spawn("dcc-ctl", &["stop"]);
    assert!(stop.wait().success());
    // Once the unknown registration is repaired to a provably absent identity,
    // drain can finish. The record's contents never enter diagnostics.
    fs::write(scripts.state.join("active/broken"), "2147483647 1\n").unwrap();
    assert!(supervisor.wait().success());
    let logs = scripts.logs();
    assert!(logs.contains("cannot determine wrapper identity"));
    assert_eq!(logs.matches("retaining active records").count(), 1);
    assert!(!logs.contains("not an identity"));
}

#[test]
fn lifecycle_real_stopped_wrapper_remains_live_until_it_exits() {
    let scripts = Scripts::new();
    scripts.ready();
    let mut command = scripts.running_command();
    scripts.wait_file("command.hit");
    command.signal("-STOP");
    let output = helper(
        &scripts,
        None,
        "for record in \"$ACTIVE\"/*; do inspect_record \"$record\"; [ \"$identity_result\" = live ]; done",
    );
    assert!(output.status.success());
    scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    scripts.wait_file("scan.hit");
    assert!(!scripts.state.join("stopping").exists());
    assert_eq!(scripts.records().len(), 1);
    scripts.release("scan");
    scripts.release("command");
    command.signal("-CONT");
    assert!(command.wait().success());
    assert!(supervisor.wait().success());
}

#[test]
fn lifecycle_shutdown_hooks_run_outside_lock_with_descriptor_closed() {
    let scripts = Scripts::new();
    scripts.ready();
    write_script(
        &scripts.share,
        "dcc-shutdown",
        "#!/bin/sh\nset -eu\n[ ! -e /proc/self/fd/9 ]\nflock -n \"$TEST_STATE/lifecycle.lock\" sh -c ': > \"$TEST_STATE/shutdown.hit\"'\n".into(),
    )
    .unwrap();
    let mut command = scripts.spawn("dcc-exec", &["true"]);
    assert!(command.wait().success());
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    assert!(supervisor.wait().success(), "{}", scripts.logs());
    scripts.wait_file("shutdown.hit");
}

#[test]
fn lifecycle_stop_now_signals_after_unlock_and_rechecks_snapshot_identity() {
    for dies_after_snapshot in [false, true] {
        let scripts = Scripts::new();
        scripts.ready();
        let mut command = scripts.running_command();
        scripts.wait_file("command.hit");
        // Observe actual signalling, and prove the target branch runs with fd9
        // closed and the lock available to a different open file description.
        scripts.edit(
            "dcc-ctl",
            "                    *) kill -TERM \"$target_pid\" 2>/dev/null || true ;;",
            "                    *) [ ! -e /proc/self/fd/9 ]; flock -n \"$LOCK\" true; : > \"$STATE/signal.hit\"; kill -TERM \"$target_pid\" 2>/dev/null || true ;;",
        );
        scripts.gate(
            "dcc-ctl",
            "        # Signals happen outside the lock.",
            "snapshot",
            false,
        );
        let mut stop = scripts.spawn("dcc-ctl", &["stop-now"]);
        scripts.wait_file("snapshot.hit");
        assert!(scripts.state.join("stopping").exists());
        if dies_after_snapshot {
            command.signal("-KILL");
            command.wait();
        }
        scripts.release("snapshot");
        assert!(stop.wait().success(), "{}", scripts.logs());
        assert_eq!(
            scripts.state.join("signal.hit").exists(),
            !dies_after_snapshot
        );
        scripts.release("command");
        if !dies_after_snapshot {
            assert_eq!(command.wait().code(), Some(143));
        }
    }
}

#[test]
fn lifecycle_real_zombie_is_retired_and_kernel_comm_with_newline_is_parsed() {
    let scripts = Scripts::new();
    let mut zombie = Process(Command::new("true").process_group(0).spawn().unwrap());
    let pid = zombie.0.id();
    let until = Instant::now() + DEADLINE;
    loop {
        let data = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        if data.rsplit_once(") ").unwrap().1.starts_with("Z ") {
            break;
        }
        assert!(Instant::now() < until, "child never reached zombie state");
        thread::sleep(Duration::from_millis(10));
    }
    let output = helper(
        &scripts,
        None,
        &format!(
            "proc_identity {pid}; [ \"$proc_state\" = Z ]; printf '%s %s\\n' {pid} \"$proc_start\" > \"$ACTIVE/zombie\"; lifecycle_lock; retire_records; lifecycle_unlock; [ \"$n\" -eq 0 ]; [ \"$arrived\" -eq 1 ]; [ ! -e \"$ACTIVE/zombie\" ]"
        ),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(zombie.wait().success());
    let output = helper(
        &scripts,
        None,
        "printf 'odd )\\nx' > /proc/self/comm; proc_identity $$; [ \"$proc_state\" != Z ]; [ -n \"$proc_start\" ]",
    );
    assert!(
        output.status.success(),
        "actual kernel comm could not be parsed"
    );
}

#[test]
fn lifecycle_supervisor_term_releases_its_lock_before_shutdown() {
    let scripts = Scripts::new();
    scripts.ready();
    scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
    write_script(
        &scripts.share,
        "dcc-shutdown",
        "#!/bin/sh\nset -eu\n[ ! -e /proc/self/fd/9 ]\nflock -n \"$TEST_STATE/lifecycle.lock\" sh -c ': > \"$TEST_STATE/shutdown.hit\"'\n".into(),
    )
    .unwrap();
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "durable"]);
    scripts.wait_file("scan.hit");
    supervisor.signal("-TERM");
    assert!(supervisor.wait().success(), "{}", scripts.logs());
    scripts.wait_file("shutdown.hit");
    let mut refused = scripts.spawn("dcc-exec", &["true"]);
    assert_eq!(refused.wait().code(), Some(253));
}

#[test]
fn lifecycle_mode_promotion_cannot_reopen_committed_drain() {
    let scripts = Scripts::new();
    scripts.ready();
    let mut previous = scripts.spawn("dcc-exec", &["true"]);
    assert!(previous.wait().success());
    scripts.gate("dcc-supervisor", "    retire_records\n", "scan", true);
    scripts.gate(
        "dcc-ctl",
        "        lifecycle_lock\n        if [ -f \"$STOPPING\" ]; then",
        "promote",
        false,
    );
    let mut supervisor = scripts.spawn("dcc-supervisor", &["--mode", "oneshot"]);
    scripts.wait_file("scan.hit");
    let mut promote = scripts.spawn("dcc-ctl", &["mode", "durable"]);
    scripts.wait_file("promote.hit");
    scripts.release("promote");
    scripts.release("scan");
    assert!(supervisor.wait().success());
    assert_eq!(promote.wait().code(), Some(253));
    assert_eq!(
        fs::read_to_string(scripts.state.join("mode")).unwrap(),
        "oneshot"
    );
}
