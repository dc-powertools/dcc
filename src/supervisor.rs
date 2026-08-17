//! In-container lifecycle supervisor assets.
//!
//! The container's PID 1 is a POSIX `sh` supervisor (`dcc-supervisor`) that owns the
//! one-shot/durable mode, startup sequencing, `postStartCommand` hook execution, a
//! readiness handshake, the active-command set, and the teardown decision. The host
//! CLI drives it through two helper scripts: `dcc-ctl` for control verbs and `dcc-exec`
//! which wraps a single user command, registers it with the supervisor, waits for
//! readiness, then runs it.
//!
//! All three scripts are generated from this module so there is a single source of
//! truth. They are **baked into the image** at `/usr/local/share/dcc/` via the build
//! context (decision 0004), so every dcc-built image carries them and they are
//! version-stamped alongside the image by the `dcc.version` label.
//!
//! Startup hooks are delivered as pre-substituted executable scripts written into
//! `<workspace>/.dcc/<profile>.rt/start-hooks/` on the host, bind-mounted read-only at
//! `/usr/local/share/dcc/rt`, and passed to the supervisor via `--start-hooks`. They
//! are NOT baked, because `postStartCommand` may contain `${localEnv:VAR}` which is only
//! resolvable at run time from the invoking user's environment (T-0028 Q3).
//!
//! Lifecycle state lives in a container-private tmpfs at `/run/dcc` (see
//! [`STATE_DIR`]), which dies with the container and is never host-backed.

use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::{
    lifecycle::{LifecycleCommand, LifecycleHooks},
    profile::ProfileName,
    workspace::Workspace,
};

/// Container-side directory where dcc bakes its runtime assets (supervisor
/// scripts) and where the `rt` bind mount delivers startup hook scripts. The
/// supervisor scripts live at `/usr/local/share/dcc/dcc-supervisor`,
/// `/usr/local/share/dcc/dcc-ctl`, `/usr/local/share/dcc/dcc-exec` (baked into
/// the image); startup hooks are bind-mounted at `/usr/local/share/dcc/rt`.
pub(crate) const DCC_SHARE: &str = "/usr/local/share/dcc";

/// Container-side mount point for the read-only startup hooks directory.
/// Only `start-hooks/` lives here now; the supervisor scripts are baked into
/// the image at [`DCC_SHARE`].
pub(crate) const RT_MOUNT: &str = "/usr/local/share/dcc/rt";

/// Container-side, container-private lifecycle state directory (tmpfs).
pub(crate) const STATE_DIR: &str = "/run/dcc";

/// Readiness status file: `0` on success, `<exit-code> <hook-name>` on failure.
pub(crate) const BOOTSTRAP_STATUS: &str = "/run/dcc/bootstrap-status";

/// Directory of per-waiter FIFOs used by `dcc-ctl wait-ready`.
const WAITERS_DIR: &str = "/run/dcc/waiters";

/// Combined startup-hook output, replayed to the user on failure.
const HOOK_LOG: &str = "/run/dcc/hook.log";

/// Exit code returned by `wait-ready` when a startup hook failed (distinct from the
/// `dcc-exec` "shutting down" code 253 and from any user command exit).
pub(crate) const EXIT_BOOTSTRAP_FAILED: i32 = 252;

/// One-shot orphan reaper: after bootstrap completes, if no command ever registers
/// within this many seconds, the supervisor exits rather than leaving the container
/// idle. Only applies to one-shot containers; durable containers never reap.
const REAPER_SECS: u32 = 10;

/// Drain poll interval in milliseconds. Passed to `sleep` as a fractional
/// second value (e.g. `sleep 0.2`), which GNU coreutils and BusyBox `sleep`
/// both support — covering glibc, Alpine, and all base images `dcc` targets.
const POLL_MS: u32 = 200;

/// Host-side runtime assets directory: `<workspace>/.dcc/<profile>.rt/`.
#[derive(Debug)]
pub(crate) struct RtDir {
    pub(crate) host_path: PathBuf,
}

impl RtDir {
    pub(crate) fn new(workspace: &Workspace, profile: &ProfileName) -> Self {
        Self {
            host_path: workspace
                .root
                .join(".dcc")
                .join(format!("{}.rt", profile.as_str())),
        }
    }

    /// Ensures the `rt` directory exists and clears any stale `start-hooks/`
    /// directory so a previous `dcc exec`'s hooks cannot leak into a build-prep
    /// container (which calls this but passes no `--start-hooks`). Idempotent.
    ///
    /// The supervisor scripts themselves are baked into the image (decision 0004)
    /// and are no longer written here; this directory now holds only the
    /// per-launch `start-hooks/` scripts written by [`Self::write_start_hooks`].
    pub(crate) fn materialize(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.host_path).with_context(|| {
            format!(
                "failed to create runtime assets directory `{}`",
                self.host_path.display()
            )
        })?;
        // Remove any stale start-hooks from a prior launch; the host rewrites them
        // fresh on each runtime launch (see [`Self::write_start_hooks`]).
        let start_hooks = self.host_path.join("start-hooks");
        if start_hooks.exists() {
            std::fs::remove_dir_all(&start_hooks).with_context(|| {
                format!(
                    "failed to remove stale start-hooks directory `{}`",
                    start_hooks.display()
                )
            })?;
        }
        Ok(())
    }

    /// Writes the `postStartCommand` hook scripts (feature hooks first in
    /// installation order, then the devcontainer hook) into `start-hooks/`. Each
    /// script is named `NN-<sanitized-source>` so lexical order is execution order.
    /// Called only on the runtime path; the build-prep path does not call this.
    ///
    /// `substitute` is applied to every hook string before emission, so the generated
    /// scripts are fully resolved — no `${containerEnv:…}` reaches the container.
    pub(crate) fn write_start_hooks(
        &self,
        feature_hooks: &[(String, LifecycleHooks)],
        config_hooks: &LifecycleHooks,
        substitute: &impl Fn(&str) -> anyhow::Result<String>,
        container_user: &str,
        workdir: &str,
    ) -> anyhow::Result<()> {
        let dir = self.host_path.join("start-hooks");
        std::fs::create_dir_all(&dir).with_context(|| {
            format!("failed to create start-hooks directory `{}`", dir.display())
        })?;

        let mut entries: Vec<(String, LifecycleCommand)> = Vec::new();
        for (feature_id, hooks) in feature_hooks {
            if let Some(cmd) = &hooks.post_start_command {
                let cmd = cmd
                    .try_substitute(substitute)
                    .with_context(|| format!("postStartCommand from feature `{feature_id}`"))?;
                entries.push((format!("feature-{feature_id}"), cmd));
            }
        }
        if let Some(cmd) = &config_hooks.post_start_command {
            let cmd = cmd
                .try_substitute(substitute)
                .with_context(|| "postStartCommand")?;
            entries.push(("devcontainer".to_string(), cmd));
        }

        for (idx, (source, cmd)) in entries.into_iter().enumerate() {
            let name = format!("{:02}-{}", idx, sanitize_source(&source));
            let path = dir.join(&name);
            let body = hook_script(&cmd, container_user, workdir, &source);
            write_script(&self.host_path.join("start-hooks"), &name, body)?;
            let _ = path; // written via write_script above
        }
        Ok(())
    }

    /// Returns true if any `postStartCommand` hook exists across features or config.
    /// Used to decide whether the host should pass `--start-hooks` at all; when false,
    /// the supervisor runs nothing and marks itself ready immediately.
    pub(crate) fn has_start_hooks(
        feature_hooks: &[(String, LifecycleHooks)],
        config_hooks: &LifecycleHooks,
    ) -> bool {
        feature_hooks
            .iter()
            .any(|(_, h)| h.post_start_command.is_some())
            || config_hooks.post_start_command.is_some()
    }

    /// Container-side path to the `start-hooks` directory, for `--start-hooks`.
    pub(crate) fn start_hooks_container_path(&self) -> String {
        format!("{RT_MOUNT}/start-hooks")
    }

    /// `--mount` argument for `docker run` (read-only bind mount of the supervisor
    /// scripts).
    pub(crate) fn mount_arg(&self) -> String {
        format!(
            "type=bind,source={},target={RT_MOUNT},readonly",
            self.host_path.display()
        )
    }
}

/// Returns the three supervisor scripts as build-context assets to be baked into
/// the image at `/usr/local/share/dcc/`. Each is `(path, content, mode)`. The
/// `COPY .dcc-generated/ /usr/local/share/dcc/` step places them, and the
/// `find … -exec chmod +x` step makes them executable.
pub(crate) fn baked_supervisor_assets() -> Vec<(String, Vec<u8>, u32)> {
    vec![
        (
            ".dcc-generated/dcc-supervisor".to_string(),
            supervisor_script().into_bytes(),
            0o755,
        ),
        (
            ".dcc-generated/dcc-ctl".to_string(),
            ctl_script().into_bytes(),
            0o755,
        ),
        (
            ".dcc-generated/dcc-exec".to_string(),
            exec_script().into_bytes(),
            0o755,
        ),
    ]
}

fn write_script(dir: &Path, name: &str, content: String) -> anyhow::Result<()> {
    let path = dir.join(name);
    std::fs::write(&path, content)
        .with_context(|| format!("failed to write runtime asset `{}`", path.display()))?;
    set_executable(&path)?;
    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).with_context(|| {
        format!(
            "failed to set executable permissions on `{}`",
            path.display()
        )
    })
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// Returns the container mode value for a `--keep` flag.
pub(crate) fn mode_value(keep: bool) -> &'static str {
    if keep {
        "durable"
    } else {
        "oneshot"
    }
}

/// Appends the `docker run` image and supervisor entrypoint argv.
///
/// Docker parses every argument before the image as a Docker-run option, so
/// supervisor flags such as `--mode` must come after the image tag.
pub(crate) fn append_run_image_and_args(
    args: &mut Vec<String>,
    image: &str,
    mode: &str,
    expect_command: bool,
    start_hooks: Option<&str>,
) {
    args.push(image.to_string());
    args.push("--mode".to_string());
    args.push(mode.to_string());
    if expect_command {
        args.push("--expect-command".to_string());
    }
    if let Some(start_hooks) = start_hooks {
        args.push("--start-hooks".to_string());
        args.push(start_hooks.to_string());
    }
}

/// Sanitizes a feature id / source label into a filesystem-safe script name suffix.
fn sanitize_source(source: &str) -> String {
    source
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// POSIX single-quote escaping for an argv element, so no user string can break out
/// of the generated hook script. `foo'bar` becomes `'foo'\''bar'`.
fn sh_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Emits a POSIX `sh` script that runs one hook (possibly parallel) as
/// `container_user` from `workdir`, teeing output to `hook.log`, and exiting with the
/// first failing status (after waiting for parallel entries to finish).
fn hook_script(cmd: &LifecycleCommand, user: &str, workdir: &str, source: &str) -> String {
    let argvs = cmd.argvs();
    if argvs.is_empty() {
        // No-op hook: exit 0 immediately.
        return "#!/bin/sh\nexit 0\n".to_string();
    }
    let mut lines = Vec::new();
    lines.push("#!/bin/sh".to_string());
    lines.push("# Auto-generated postStartCommand hook.".to_string());
    lines.push(format!("# source: {source}"));
    lines.push("set -eu".to_string());
    lines.push(format!("HOOK_LOG={HOOK_LOG}"));
    // Run as the configured container user from the configured workdir.
    lines.push(format!("USER={}", sh_quote(user)));
    lines.push(format!("WORKDIR={}", sh_quote(workdir)));

    if let [argv] = argvs.as_slice() {
        // Single command: run it via `su`/`cd`, tee output.
        let cmd_str = argv
            .iter()
            .map(|a| sh_quote(a))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(format!(
            "cd \"$WORKDIR\" 2>/dev/null || cd /\n\
             {cmd_str} 2>&1 | tee -a \"$HOOK_LOG\"\n\
             exit ${{PIPESTATUS:-$?}}\n"
        ));
    } else {
        // Parallel: background each, wait, return first failure.
        lines.push("cd \"$WORKDIR\" 2>/dev/null || cd /".to_string());
        lines.push("first_status=0".to_string());
        for argv in &argvs {
            let cmd_str = argv
                .iter()
                .map(|a| sh_quote(a))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(format!("({cmd_str} 2>&1 | tee -a \"$HOOK_LOG\") &"));
        }
        lines.push("for job in $(jobs -p); do".to_string());
        lines.push("  wait \"$job\" || first_status=$?".to_string());
        lines.push("done".to_string());
        lines.push("exit \"$first_status\"".to_string());
    }
    lines.join("\n") + "\n"
}

// ---------------------------------------------------------------------------
// Script sources
// ---------------------------------------------------------------------------

/// PID 1 supervisor. Owns mode, startup hooks, readiness, the active-command set,
/// drain/teardown, and the one-shot orphan reaper.
fn supervisor_script() -> String {
    r#"#!/bin/sh
# dcc container lifecycle supervisor (PID 1).
# Owns mode, startup hooks, readiness, the active-command set, and teardown.
set -eu

STATE="__STATE_DIR__"
ACTIVE="$STATE/active"
MODE_FILE="$STATE/mode"
STOPPING="$STATE/stopping"
STATUS="__STATUS__"
WAITERS="__WAITERS__"
HOOK_LOG="__HOOK_LOG__"
SHUTDOWN="__DCC_SHARE__/dcc-shutdown"

mkdir -p "$ACTIVE" "$WAITERS"

# --- Argument parsing: --mode <oneshot|durable> [--expect-command]
#                        [--start-hooks <dir>] ---
mode="oneshot"
expect_command=0
start_hooks=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --mode)
            mode="${2:-oneshot}"; shift 2 || shift
            ;;
        --expect-command)
            expect_command=1; shift
            ;;
        --start-hooks)
            start_hooks="${2:-}"; shift 2 || shift
            ;;
        --)
            shift; break
            ;;
        *)
            echo "dcc-supervisor: unknown argument \`$1'" >&2; shift
            ;;
    esac
done

printf '%s' "$mode" > "$MODE_FILE"

# --- Bootstrap: run startup hooks, then write bootstrap-status and signal
# waiters. A failure writes a non-zero status and the failing hook name but
# does NOT exit — the container stays alive so the harness can observe and
# report the failure. ---
bootstrap() {
    if [ -n "$start_hooks" ] && [ -d "$start_hooks" ]; then
        : > "$HOOK_LOG" 2>/dev/null || true
        for f in "$start_hooks"/*; do
            [ -f "$f" ] || continue
            name=$(basename "$f")
            # Run the hook; capture its exit status.
            set +e
            sh "$f"
            status=$?
            set -e
            if [ "$status" -ne 0 ]; then
                printf '%s %s' "$status" "$name" > "$STATUS.tmp"
                mv "$STATUS.tmp" "$STATUS"
                signal_waiters
                return 0
            fi
        done
    fi
    printf '0' > "$STATUS.tmp"
    mv "$STATUS.tmp" "$STATUS"
    signal_waiters
}

# Signal every waiter FIFO without blocking. Opening read-write (`<>`) never
# blocks even when the reader has died and left an orphaned FIFO, so a dead
# waiter cannot wedge PID 1.
signal_waiters() {
    for f in "$WAITERS"/*; do
        [ -p "$f" ] || continue
        { exec 3<>"$f"; printf 'go\n' >&3; exec 3>&-; } 2>/dev/null || true
    done
}

active_count() {
    if [ ! -d "$ACTIVE" ]; then
        echo 0
        return
    fi
    n=0
    for f in "$ACTIVE"/*; do
        [ -f "$f" ] || continue
        n=$((n + 1))
    done
    echo "$n"
}

# Run bootstrap in the background so the main loop can start polling
# immediately. bootstrap writes STATUS and signals waiters when done.
bootstrap &
bootstrap_done=0
arrived=0

# SIGTERM (e.g. docker stop / docker kill) => run shutdown and exit cleanly.
trap 'run_shutdown; exit 0' TERM

run_shutdown() {
    if [ -x "$SHUTDOWN" ]; then
        "$SHUTDOWN" || true
    fi
}

# Track when bootstrap completed, for the one-shot orphan reaper.
bootstrap_finished=0
bootstrap_elapsed=0

while true; do
    # Detect bootstrap completion once.
    if [ "$bootstrap_finished" -eq 0 ] && [ -f "$STATUS" ]; then
        bootstrap_finished=1
        bootstrap_started_sec=$(date +%s)
    fi

    n=$(active_count)
    [ "$n" -gt 0 ] && arrived=1

    if [ -f "$STOPPING" ]; then
        if [ "$n" -eq 0 ]; then
            run_shutdown
            exit 0
        fi
    elif [ "$mode" = "oneshot" ]; then
        if [ "$arrived" -eq 1 ] && [ "$n" -eq 0 ]; then
            run_shutdown
            exit 0
        fi
        # Orphan reaper: no command ever registered and bootstrap finished
        # more than REAPER_SECS ago. Only for one-shot containers.
        if [ "$arrived" -eq 0 ] && [ "$bootstrap_finished" -eq 1 ]; then
            now=$(date +%s)
            elapsed=$((now - bootstrap_started_sec))
            if [ "$elapsed" -ge "__REAPER_SECS__" ]; then
                run_shutdown
                exit 0
            fi
        fi
    fi
    # Durable without stopping: stay alive forever.
    sleep __POLL_SECS__
done
"#
    .replace("__STATE_DIR__", STATE_DIR)
    .replace("__DCC_SHARE__", DCC_SHARE)
    .replace("__STATUS__", BOOTSTRAP_STATUS)
    .replace("__WAITERS__", WAITERS_DIR)
    .replace("__HOOK_LOG__", HOOK_LOG)
    .replace("__REAPER_SECS__", &REAPER_SECS.to_string())
    .replace("__POLL_SECS__", &format!("{:.1}", POLL_MS as f64 / 1000.0))
}

/// Control script. Invoked by the host CLI via `docker exec dcc-ctl <verb>`.
fn ctl_script() -> String {
    r#"#!/bin/sh
# dcc control script. Drives the supervisor from the host CLI.
set -eu

STATE="__STATE_DIR__"
MODE_FILE="$STATE/mode"
STOPPING="$STATE/stopping"
STATUS="__STATUS__"
WAITERS="__WAITERS__"
HOOK_LOG="__HOOK_LOG__"

case "${1:-}" in
    mode)
        # Promote an already-running container to durable.
        printf '%s' "${2:-durable}" > "$MODE_FILE"
        ;;
    stop)
        # Graceful: stop accepting new commands, drain, then exit.
        mkdir -p "$STATE"
        : > "$STOPPING"
        ;;
    stop-now)
        # Forceful: signal registered commands, run shutdown hooks, exit.
        mkdir -p "$STATE"
        : > "$STOPPING"
        if [ -d "$STATE/active" ]; then
            for f in "$STATE/active"/*; do
                [ -f "$f" ] || continue
                pid=$(cat "$f" 2>/dev/null || true)
                [ -n "$pid" ] && kill -TERM "$pid" 2>/dev/null || true
            done
        fi
        ;;
    wait-ready)
        # Block until the supervisor signals bootstrap completion, then return
        # its status. Registers a per-waiter FIFO BEFORE checking status so a
        # signal cannot be lost. If status already exists (steady state), return
        # immediately without blocking.
        mkdir -p "$WAITERS"
        id="wr-$$-$(date +%s%N 2>/dev/null || date +%s)"
        fifo="$WAITERS/$id"
        mkfifo "$fifo" 2>/dev/null || true
        if [ -f "$STATUS" ]; then
            rm -f "$fifo"
        else
            # Block until the supervisor signals. `read` blocks in open(2).
            read -r _line < "$fifo" 2>/dev/null || true
            rm -f "$fifo"
        fi
        if [ ! -f "$STATUS" ]; then
            echo "dcc-ctl: bootstrap status never appeared" >&2
            exit 252
        fi
        contents=$(cat "$STATUS")
        case "$contents" in
            0)
                exit 0
                ;;
            *)
                # Failure: `<exit-code> <hook-name>`. Print the failing hook and
                # the tail of the hook log, then exit 252.
                hook_name=${contents#* }
                echo "dcc: startup hook \`$hook_name' failed (status ${contents%% *})" >&2
                if [ -f "$HOOK_LOG" ]; then
                    tail -n 20 "$HOOK_LOG" >&2 2>/dev/null || true
                fi
                exit 252
                ;;
        esac
        ;;
    *)
        echo "dcc-ctl: unknown verb \`${1:-}\` (expected: mode <oneshot|durable> | stop | stop-now | wait-ready)" >&2
        exit 2
        ;;
esac
"#
    .replace("__STATE_DIR__", STATE_DIR)
    .replace("__DCC_SHARE__", DCC_SHARE)
    .replace("__STATUS__", BOOTSTRAP_STATUS)
    .replace("__WAITERS__", WAITERS_DIR)
    .replace("__HOOK_LOG__", HOOK_LOG)
}

/// Command wrapper. Registers a command, waits for readiness, runs it, deregisters
/// on any exit. Exits with the command's status.
fn exec_script() -> String {
    r#"#!/bin/sh
# dcc command wrapper. Registers one command with the supervisor, waits for
# readiness, runs it, and deregisters on exit (normal, error, or signal).
# Exits with the command's status.
set -eu

STATE="__STATE_DIR__"
ACTIVE="$STATE/active"
STOPPING="$STATE/stopping"
CTL="__DCC_SHARE__/dcc-ctl"

if [ "$#" -lt 1 ]; then
    echo "dcc-exec: missing command argument" >&2
    exit 64
fi

# Refuse new work once a graceful stop is in progress.
if [ -f "$STOPPING" ]; then
    echo "dcc: container is shutting down; retry on a fresh container" >&2
    exit 253
fi

mkdir -p "$ACTIVE"
id=$(printf 'dcc-%s-%s' "$$" "$(date +%s%N 2>/dev/null || date +%s)")
record="$ACTIVE/$id"
printf '%s' "$$" > "$record"

# Deregister on any exit, then propagate the child's status.
status=0
trap 'rm -f "$record"; exit "$status"' EXIT
trap 'rm -f "$record"; status=130; exit 130' INT
trap 'rm -f "$record"; status=143; exit 143' TERM

# Wait for the supervisor to finish bootstrap (hooks) before running the
# command. The active record is already created, so the supervisor cannot
# drain out from under us while we wait. wait-ready exits 0 on success, 252
# if a startup hook failed.
"$CTL" wait-ready || exit $?

# Run the command with set -e disabled so a non-zero exit is captured rather than
# aborting the wrapper before status=$? runs.
set +e
"$@"
status=$?
set -e
exit "$status"
"#
    .replace("__STATE_DIR__", STATE_DIR)
    .replace("__DCC_SHARE__", DCC_SHARE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheDir;
    use crate::lifecycle::LifecycleCommand;

    #[test]
    fn supervisor_script_has_shebang_and_set_eu() {
        let s = supervisor_script();
        assert!(s.starts_with("#!/bin/sh\n"));
        assert!(s.contains("set -eu"));
    }

    #[test]
    fn supervisor_script_parses_mode_and_expect_command_and_start_hooks() {
        let s = supervisor_script();
        assert!(s.contains("--mode"));
        assert!(s.contains("--expect-command"));
        assert!(s.contains("--start-hooks"));
        assert!(s.contains("case \"$1\" in"));
    }

    #[test]
    fn supervisor_script_writes_bootstrap_status_and_signals_waiters() {
        let s = supervisor_script();
        assert!(s.contains(BOOTSTRAP_STATUS));
        assert!(s.contains("signal_waiters"));
        // The <> open that avoids wedging on orphaned FIFOs.
        assert!(s.contains("exec 3<>"));
    }

    #[test]
    fn supervisor_script_has_reaper_not_grace() {
        let s = supervisor_script();
        assert!(s.contains("REAPER_SECS"));
        assert!(s.contains("arrived"));
        // The old time-based grace must be gone.
        assert!(!s.contains("STARTUP_GRACE_SECS"));
        assert!(!s.contains("PRIMED"));
        assert!(!s.contains("primed"));
    }

    #[test]
    fn supervisor_script_uses_fractional_sleep() {
        let s = supervisor_script();
        assert!(
            s.contains("sleep 0.2"),
            "expected `sleep 0.2` (200ms as fractional seconds), got: {s}"
        );
    }

    #[test]
    fn ctl_script_supports_mode_stop_stop_now_and_wait_ready() {
        let s = ctl_script();
        assert!(s.contains("mode)"));
        assert!(s.contains("stop)"));
        assert!(s.contains("stop-now)"));
        assert!(s.contains("wait-ready)"));
        assert!(s.contains("STOPPING"));
        assert!(s.contains(BOOTSTRAP_STATUS));
        // Failure exit code 252 for bootstrap failure.
        assert!(s.contains("exit 252"));
    }

    #[test]
    fn ctl_script_wait_ready_registers_fifo_before_status_check() {
        let s = ctl_script();
        // mkfifo must appear before the status file check, so a signal cannot
        // be lost. We verify the ordering by checking both are present and
        // that the fast-path status check follows the mkfifo.
        let mkfifo_pos = s.find("mkfifo").expect("mkfifo present");
        let status_check_pos = s
            .find("if [ -f \"$STATUS\" ]")
            .expect("status check present");
        assert!(
            mkfifo_pos < status_check_pos,
            "mkfifo must come before the status check (lossless ordering)"
        );
    }

    #[test]
    fn exec_script_refuses_when_stopping() {
        let s = exec_script();
        assert!(s.contains("STOPPING"));
        assert!(s.contains("shutting down"));
        assert!(s.contains("253"));
    }

    #[test]
    fn scripts_reference_dcc_share_not_rt_mount() {
        // The supervisor and helper scripts are baked at DCC_SHARE, not RT_MOUNT.
        // dcc-exec calls dcc-ctl, and the supervisor calls dcc-shutdown, both at
        // DCC_SHARE. RT_MOUNT is only for start-hooks.
        let sup = supervisor_script();
        let exec = exec_script();
        assert!(
            sup.contains(&format!("{DCC_SHARE}/dcc-shutdown")),
            "supervisor should reference dcc-shutdown at DCC_SHARE"
        );
        assert!(
            !sup.contains(&format!("{RT_MOUNT}/dcc-shutdown")),
            "supervisor should not reference dcc-shutdown at RT_MOUNT"
        );
        assert!(
            exec.contains(&format!("{DCC_SHARE}/dcc-ctl")),
            "dcc-exec should reference dcc-ctl at DCC_SHARE"
        );
        assert!(
            !exec.contains(&format!("{RT_MOUNT}/dcc-ctl")),
            "dcc-exec should not reference dcc-ctl at RT_MOUNT"
        );
    }

    #[test]
    fn exec_script_registers_then_waits_then_runs() {
        let s = exec_script();
        assert!(s.contains("ACTIVE"));
        assert!(s.contains("record"));
        assert!(s.contains("rm -f \"$record\""));
        assert!(s.contains("\"$@\""));
        // Must call wait-ready before running the command.
        let wait_pos = s.find("wait-ready").expect("wait-ready present");
        let run_pos = s.find("\"$@\"").expect("command exec present");
        assert!(
            wait_pos < run_pos,
            "wait-ready must come before the command exec"
        );
    }

    #[test]
    fn mode_value_maps_keep() {
        assert_eq!(mode_value(true), "durable");
        assert_eq!(mode_value(false), "oneshot");
    }

    #[test]
    fn append_run_image_and_args_places_supervisor_args_after_image() {
        let mut args = vec![
            "--name".to_string(),
            "dcc-test".to_string(),
            "--entrypoint".to_string(),
            format!("{DCC_SHARE}/dcc-supervisor"),
        ];
        append_run_image_and_args(
            &mut args,
            "dcc-image",
            "oneshot",
            true,
            Some("/usr/local/share/dcc/rt/start-hooks"),
        );

        assert_eq!(
            args,
            vec![
                "--name",
                "dcc-test",
                "--entrypoint",
                "/usr/local/share/dcc/dcc-supervisor",
                "dcc-image",
                "--mode",
                "oneshot",
                "--expect-command",
                "--start-hooks",
                "/usr/local/share/dcc/rt/start-hooks",
            ]
        );
    }

    #[test]
    fn rt_dir_is_sibling_of_cache_root() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "test".to_string(),
        };
        let profile = ProfileName::new("dev");
        let cache = CacheDir::new(&ws, &profile);
        let rt = RtDir::new(&ws, &profile);
        assert_eq!(cache.host_path, tmp.path().join(".dcc").join("dev"));
        assert_eq!(rt.host_path, tmp.path().join(".dcc").join("dev.rt"));
        assert!(!rt.host_path.starts_with(&cache.host_path));
    }

    #[test]
    fn materialize_creates_dir_and_clears_stale_start_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "test".to_string(),
        };
        let profile = ProfileName::new("dev");
        let rt = RtDir::new(&ws, &profile);

        // Plant a stale start-hooks directory to verify it is cleared.
        let stale = rt.host_path.join("start-hooks");
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("00-stale"), "#!/bin/sh\nexit 0\n").unwrap();

        rt.materialize().unwrap();

        // The rt directory exists, but the supervisor scripts are no longer
        // written here (they are baked into the image).
        assert!(rt.host_path.is_dir(), "rt directory should exist");
        assert!(
            !rt.host_path.join("dcc-supervisor").exists(),
            "supervisor scripts are baked into the image, not written to rt"
        );
        // The stale start-hooks directory should be gone.
        assert!(!stale.exists(), "stale start-hooks should be removed");
    }

    #[test]
    fn baked_supervisor_assets_emits_three_executable_scripts() {
        let assets = baked_supervisor_assets();
        assert_eq!(assets.len(), 3, "expected three supervisor scripts");
        let names: Vec<&str> = assets.iter().map(|(p, _, _)| p.as_str()).collect();
        assert!(
            names.iter().all(|p| p.starts_with(".dcc-generated/dcc-")),
            "assets should target .dcc-generated/, got: {names:?}"
        );
        for (path, content, mode) in &assets {
            let text = std::str::from_utf8(content).unwrap();
            assert!(
                text.starts_with("#!/bin/sh\n"),
                "{path} should have a shebang"
            );
            assert_eq!(*mode, 0o755, "{path} should be executable");
        }
    }

    #[test]
    fn mount_arg_is_read_only() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "test".to_string(),
        };
        let profile = ProfileName::new("dev");
        let rt = RtDir::new(&ws, &profile);
        let arg = rt.mount_arg();
        assert!(arg.contains("readonly"));
        assert!(arg.contains("type=bind"));
        assert!(arg.contains(RT_MOUNT));
    }

    #[test]
    fn sh_quote_escapes_single_quotes() {
        assert_eq!(sh_quote("simple"), "'simple'");
        assert_eq!(sh_quote("foo'bar"), "'foo'\\''bar'");
        assert_eq!(sh_quote(""), "''");
        assert_eq!(sh_quote("a b"), "'a b'");
    }

    #[test]
    fn sanitize_source_replaces_unsafe_chars() {
        assert_eq!(
            sanitize_source("ghcr.io/devcontainers/features/node:1"),
            "ghcr.io_devcontainers_features_node_1"
        );
        assert_eq!(sanitize_source("./local-feat"), "._local-feat");
        assert_eq!(sanitize_source("devcontainer"), "devcontainer");
    }

    #[test]
    fn has_start_hooks_detects_post_start() {
        let config_hooks = LifecycleHooks {
            post_start_command: Some(LifecycleCommand::Shell("echo".to_string())),
            ..Default::default()
        };
        assert!(RtDir::has_start_hooks(&[], &config_hooks));

        let empty = LifecycleHooks::default();
        assert!(!RtDir::has_start_hooks(&[], &empty));

        let feature_hooks = vec![(
            "node".to_string(),
            LifecycleHooks {
                post_start_command: Some(LifecycleCommand::Shell("echo".to_string())),
                ..Default::default()
            },
        )];
        assert!(RtDir::has_start_hooks(&feature_hooks, &empty));
    }

    #[test]
    fn write_start_hooks_emits_scripts_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "test".to_string(),
        };
        let profile = ProfileName::new("dev");
        let rt = RtDir::new(&ws, &profile);
        rt.materialize().unwrap();

        let feature_hooks = vec![(
            "ghcr.io/devcontainers/features/node:1".to_string(),
            LifecycleHooks {
                post_start_command: Some(LifecycleCommand::Shell("echo feature".to_string())),
                ..Default::default()
            },
        )];
        let config_hooks = LifecycleHooks {
            post_start_command: Some(LifecycleCommand::Exec(vec![
                "echo".to_string(),
                "dev".to_string(),
            ])),
            ..Default::default()
        };
        let substitute = |s: &str| Ok(s.to_string());
        rt.write_start_hooks(
            &feature_hooks,
            &config_hooks,
            &substitute,
            "dev",
            "/workspace",
        )
        .unwrap();

        let dir = rt.host_path.join("start-hooks");
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap())
            .collect();
        entries.sort_by_key(|e| e.file_name());
        assert_eq!(entries.len(), 2);
        // Feature hook first (00), devcontainer hook second (01).
        assert!(entries[0]
            .file_name()
            .to_str()
            .unwrap()
            .starts_with("00-feature-"));
        assert!(entries[1]
            .file_name()
            .to_str()
            .unwrap()
            .starts_with("01-devcontainer"));

        let feature_script = std::fs::read_to_string(entries[0].path()).unwrap();
        assert!(feature_script.contains("echo feature"));
        let dc_script = std::fs::read_to_string(entries[1].path()).unwrap();
        assert!(dc_script.contains("'echo'"));
        assert!(dc_script.contains("'dev'"));
    }

    #[test]
    fn write_start_hooks_no_hooks_creates_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "test".to_string(),
        };
        let profile = ProfileName::new("dev");
        let rt = RtDir::new(&ws, &profile);
        rt.materialize().unwrap();

        let empty = LifecycleHooks::default();
        let substitute = |s: &str| Ok(s.to_string());
        rt.write_start_hooks(&[], &empty, &substitute, "root", "/")
            .unwrap();

        let dir = rt.host_path.join("start-hooks");
        assert!(dir.exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn write_start_hooks_applies_substitution() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "test".to_string(),
        };
        let profile = ProfileName::new("dev");
        let rt = RtDir::new(&ws, &profile);
        rt.materialize().unwrap();

        let config_hooks = LifecycleHooks {
            post_start_command: Some(LifecycleCommand::Shell("echo ${HELLO}".to_string())),
            ..Default::default()
        };
        let substitute = |s: &str| Ok(s.replace("${HELLO}", "world"));
        rt.write_start_hooks(&[], &config_hooks, &substitute, "root", "/")
            .unwrap();

        let dir = rt.host_path.join("start-hooks");
        let script = std::fs::read_dir(&dir).unwrap().next().unwrap().unwrap();
        let body = std::fs::read_to_string(script.path()).unwrap();
        assert!(body.contains("echo world"));
        assert!(!body.contains("${HELLO}"));
    }

    // --- End-to-end script execution tests (require a real /bin/sh) ---

    /// Check that bootstrap-status gets written with `0` when there
    /// are no hooks, and that the supervisor would drain in oneshot mode.
    #[cfg(unix)]
    #[test]
    fn bootstrap_status_zero_with_no_hooks() {
        let tmp = tempfile::tempdir().unwrap();
        let state_dir = tmp.path().join("run/dcc");
        std::fs::create_dir_all(&state_dir).unwrap();

        let script = supervisor_script().replace(STATE_DIR, state_dir.to_string_lossy().as_ref());
        let script_path = tmp.path().join("dcc-supervisor");
        std::fs::write(&script_path, &script).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut cmd = std::process::Command::new("sh");
        cmd.arg(&script_path).arg("--mode").arg("oneshot");
        let mut child = cmd.spawn().unwrap();

        // Wait for bootstrap-status to appear.
        let status_path = state_dir.join("bootstrap-status");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if status_path.exists() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                // kill the child
                let _ = child.kill();
                let _ = child.wait();
                panic!("bootstrap-status not written within 5s");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let contents = std::fs::read_to_string(&status_path).unwrap();
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(contents, "0");
    }

    /// A failing hook writes a non-zero status with the hook name.
    #[cfg(unix)]
    #[test]
    fn bootstrap_status_failure_with_failing_hook() {
        let tmp = tempfile::tempdir().unwrap();
        let state_dir = tmp.path().join("run/dcc");
        std::fs::create_dir_all(&state_dir).unwrap();
        let hooks_dir = tmp.path().join("start-hooks");
        std::fs::create_dir_all(&hooks_dir).unwrap();
        // A hook that fails with exit 42.
        std::fs::write(hooks_dir.join("00-fail"), "#!/bin/sh\nexit 42\n").unwrap();

        let script = supervisor_script().replace(STATE_DIR, state_dir.to_string_lossy().as_ref());
        let script_path = tmp.path().join("dcc-supervisor");
        std::fs::write(&script_path, &script).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut cmd = std::process::Command::new("sh");
        cmd.arg(&script_path)
            .arg("--mode")
            .arg("oneshot")
            .arg("--start-hooks")
            .arg(&hooks_dir);
        let mut child = cmd.spawn().unwrap();

        let status_path = state_dir.join("bootstrap-status");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if status_path.exists() {
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("bootstrap-status not written within 5s");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let contents = std::fs::read_to_string(&status_path).unwrap();
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            contents.starts_with("42 "),
            "expected '42 00-fail', got '{contents}'"
        );
        assert!(contents.contains("00-fail"));
    }

    /// wait-ready blocks until bootstrap-status is written, then exits 0.
    #[cfg(unix)]
    #[test]
    fn wait_ready_blocks_then_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let state_dir = tmp.path().join("run/dcc");
        std::fs::create_dir_all(&state_dir).unwrap();
        let waiters_dir = state_dir.join("waiters");
        std::fs::create_dir_all(&waiters_dir).unwrap();

        let ctl = ctl_script().replace(STATE_DIR, state_dir.to_string_lossy().as_ref());
        let ctl_path = tmp.path().join("dcc-ctl");
        std::fs::write(&ctl_path, &ctl).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&ctl_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        // Start wait-ready in a thread; it should block.
        let ctl_clone = ctl_path.clone();
        let handle = std::thread::spawn(move || {
            std::process::Command::new("sh")
                .arg(&ctl_clone)
                .arg("wait-ready")
                .status()
                .unwrap()
                .code()
                .unwrap()
        });

        // Give it time to block, then write status=0.
        std::thread::sleep(std::time::Duration::from_millis(200));
        std::fs::write(state_dir.join("bootstrap-status.tmp"), "0").unwrap();
        std::fs::rename(
            state_dir.join("bootstrap-status.tmp"),
            state_dir.join("bootstrap-status"),
        )
        .unwrap();
        // Signal waiters (simulate supervisor).
        for f in std::fs::read_dir(&waiters_dir).unwrap() {
            let f = f.unwrap().path();
            if f.exists() {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(format!(
                        "exec 3<>'{}'; printf 'go\\n' >&3; exec 3>&-",
                        f.display()
                    ))
                    .status()
                    .ok();
            }
        }

        let exit_code = handle.join().unwrap();
        assert_eq!(exit_code, 0);
    }

    /// wait-ready returns immediately (fast path) when status already exists.
    #[cfg(unix)]
    #[test]
    fn wait_ready_fast_path_when_status_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let state_dir = tmp.path().join("run/dcc");
        std::fs::create_dir_all(&state_dir).unwrap();
        // Status already written.
        std::fs::write(state_dir.join("bootstrap-status"), "0").unwrap();

        let ctl = ctl_script().replace(STATE_DIR, state_dir.to_string_lossy().as_ref());
        let ctl_path = tmp.path().join("dcc-ctl");
        std::fs::write(&ctl_path, &ctl).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&ctl_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let start = std::time::Instant::now();
        let exit_code = std::process::Command::new("sh")
            .arg(&ctl_path)
            .arg("wait-ready")
            .status()
            .unwrap()
            .code()
            .unwrap();
        let elapsed = start.elapsed();
        assert_eq!(exit_code, 0);
        // Should return in well under 1 second (no blocking).
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "took {elapsed:?}"
        );
    }

    /// wait-ready exits 252 when a hook failed, and prints the failing hook name.
    #[cfg(unix)]
    #[test]
    fn wait_ready_exits_252_on_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let state_dir = tmp.path().join("run/dcc");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::fs::write(state_dir.join("bootstrap-status"), "7 00-broken").unwrap();
        std::fs::write(state_dir.join("hook.log"), "some output\n").unwrap();

        let ctl = ctl_script().replace(STATE_DIR, state_dir.to_string_lossy().as_ref());
        let ctl_path = tmp.path().join("dcc-ctl");
        std::fs::write(&ctl_path, &ctl).unwrap();
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&ctl_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        let output = std::process::Command::new("sh")
            .arg(&ctl_path)
            .arg("wait-ready")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(252));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("00-broken"), "stderr: {stderr}");
    }
}
