//! In-container lifecycle supervisor assets.
//!
//! The container's PID 1 is a POSIX `sh` supervisor (`dcc-supervisor`) that owns the
//! one-shot/durable mode, the active-command set, the teardown decision, and (for
//! `--now`) shutdown. The host CLI drives it through two helper scripts that also live
//! in the read-only `rt` bind mount: `dcc-ctl` for control verbs and `dcc-exec` which
//! wraps a single user command and registers/deregisters it with the supervisor.
//!
//! All three scripts are generated from this module so there is a single source of
//! truth. They are materialized into `<workspace>/.dcc/<profile>.rt/` on the host (a
//! sibling of the cache root, outside the `/cache` mount) and bind-mounted read-only at
//! `/usr/local/share/dcc/rt`. Container-side code can execute them but cannot modify
//! them.
//!
//! Lifecycle state lives in a container-private tmpfs at `/run/dcc` (see
//! [`STATE_DIR`]), which dies with the container and is never host-backed.

use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::{profile::ProfileName, workspace::Workspace};

/// Container-side mount point for the read-only runtime assets directory.
pub(crate) const RT_MOUNT: &str = "/usr/local/share/dcc/rt";

/// Container-side, container-private lifecycle state directory (tmpfs).
pub(crate) const STATE_DIR: &str = "/run/dcc";

/// Environment variable carrying the initial container mode (`oneshot` | `durable`).
pub(crate) const MODE_ENV: &str = "DCC_MODE";

/// Startup grace period in seconds: the supervisor will not drain-exit before this
/// elapses even if the active set is empty, so the first `docker exec` has time to
/// register.
const STARTUP_GRACE_SECS: u32 = 60;

/// Drain poll interval in milliseconds.
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

    /// Creates the directory and writes the three supervisor scripts, executable.
    /// Idempotent: safe to call on every launch.
    pub(crate) fn materialize(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.host_path).with_context(|| {
            format!(
                "failed to create runtime assets directory `{}`",
                self.host_path.display()
            )
        })?;
        write_script(&self.host_path, "dcc-supervisor", supervisor_script())?;
        write_script(&self.host_path, "dcc-ctl", ctl_script())?;
        write_script(&self.host_path, "dcc-exec", exec_script())?;
        Ok(())
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

/// Returns the container mode env value for a `--keep` flag.
pub(crate) fn mode_env_value(keep: bool) -> &'static str {
    if keep {
        "durable"
    } else {
        "oneshot"
    }
}

// ---------------------------------------------------------------------------
// Script sources
// ---------------------------------------------------------------------------

/// PID 1 supervisor. Owns mode, the active-command set, drain/teardown, and shutdown.
fn supervisor_script() -> String {
    // Built with string replacement rather than `format!` because the script body
    // contains shell `printf`/`date` percent specifiers that Rust's format parser would
    // try to interpret.
    r#"#!/bin/sh
# dcc container lifecycle supervisor (PID 1).
# Owns mode, the active-command set, and the teardown decision.
set -eu

STATE="__STATE_DIR__"
ACTIVE="$STATE/active"
MODE_FILE="$STATE/mode"
STOPPING="$STATE/stopping"
PRIMED="$STATE/primed"
SHUTDOWN="__RT_MOUNT__/dcc-shutdown"

mkdir -p "$ACTIVE"

# Initial mode comes from the host at docker run time.
mode="${DCC_MODE:-oneshot}"
printf '%s' "$mode" > "$MODE_FILE"

started=$(date +%s)

active_count() {
    # Count regular files in the active directory. Empty or absent => 0.
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

should_exit() {
    count=$(active_count)
    # Rule 2: never drain-exit before the first command registers or the startup
    # grace period elapses, whichever comes first.
    if [ ! -f "$PRIMED" ]; then
        now=$(date +%s)
        if [ $((now - started)) -lt __STARTUP_GRACE_SECS__ ]; then
            return 1
        fi
    fi
    # Rule 1: active set must be empty.
    [ "$count" -eq 0 ] || return 1
    # Rule 3: exit if one-shot, or if a graceful stop has been requested.
    cur=$(cat "$MODE_FILE" 2>/dev/null || echo oneshot)
    [ "$cur" = "oneshot" ] && return 0
    [ -f "$STOPPING" ] && return 0
    return 1
}

run_shutdown() {
    if [ -x "$SHUTDOWN" ]; then
        "$SHUTDOWN" || true
    fi
}

# SIGTERM (e.g. docker stop / docker kill) => run shutdown and exit cleanly.
trap 'run_shutdown; exit 0' TERM

while true; do
    if should_exit; then
        run_shutdown
        exit 0
    fi
    sleep __POLL_MS__
done
"#
    .replace("__STATE_DIR__", STATE_DIR)
    .replace("__RT_MOUNT__", RT_MOUNT)
    .replace("__STARTUP_GRACE_SECS__", &STARTUP_GRACE_SECS.to_string())
    .replace("__POLL_MS__", &POLL_MS.to_string())
}

/// Control script. Invoked by the host CLI via `docker exec dcc-ctl <verb>`.
fn ctl_script() -> String {
    r#"#!/bin/sh
# dcc control script. Drives the supervisor from the host CLI.
set -eu

STATE="__STATE_DIR__"
MODE_FILE="$STATE/mode"
STOPPING="$STATE/stopping"

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
    *)
        echo "dcc-ctl: unknown verb `${1:-}` (expected: mode <oneshot|durable> | stop | stop-now)" >&2
        exit 2
        ;;
esac
"#
    .replace("__STATE_DIR__", STATE_DIR)
}

/// Command wrapper. Registers a command, runs it, deregisters on any exit.
fn exec_script() -> String {
    r#"#!/bin/sh
# dcc command wrapper. Registers one command with the supervisor, runs it, and
# deregisters on exit (normal, error, or signal). Exits with the command's status.
set -eu

STATE="__STATE_DIR__"
ACTIVE="$STATE/active"
STOPPING="$STATE/stopping"
PRIMED="__STATE_DIR__/primed"

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
: > "$PRIMED" 2>/dev/null || true
printf '%s' "$$" > "$record"

# Deregister on any exit, then propagate the child's status.
status=0
trap 'rm -f "$record"; exit "$status"' EXIT
trap 'rm -f "$record"; exit 130' INT
trap 'rm -f "$record"; exit 143' TERM

"$@"
status=$?
exit "$status"
"#
    .replace("__STATE_DIR__", STATE_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheDir;

    #[test]
    fn supervisor_script_has_shebang_and_set_eu() {
        let s = supervisor_script();
        assert!(s.starts_with("#!/bin/sh\n"));
        assert!(s.contains("set -eu"));
    }

    #[test]
    fn ctl_script_supports_mode_stop_and_stop_now() {
        let s = ctl_script();
        assert!(s.contains("mode)"));
        assert!(s.contains("stop)"));
        assert!(s.contains("stop-now)"));
        assert!(s.contains("STOPPING"));
    }

    #[test]
    fn exec_script_refuses_when_stopping() {
        let s = exec_script();
        assert!(s.contains("STOPPING"));
        assert!(s.contains("shutting down"));
        assert!(s.contains("253"));
    }

    #[test]
    fn exec_script_registers_and_deregisters() {
        let s = exec_script();
        assert!(s.contains("ACTIVE"));
        assert!(s.contains("record"));
        assert!(s.contains("rm -f \"$record\""));
        assert!(s.contains("\"$@\""));
    }

    #[test]
    fn mode_env_value_maps_keep() {
        assert_eq!(mode_env_value(true), "durable");
        assert_eq!(mode_env_value(false), "oneshot");
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
        // The rt dir is outside the cache root, so it is not under the /cache mount.
        assert!(!rt.host_path.starts_with(&cache.host_path));
    }

    #[test]
    fn materialize_writes_three_executable_scripts() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "test".to_string(),
        };
        let profile = ProfileName::new("dev");
        let rt = RtDir::new(&ws, &profile);
        rt.materialize().unwrap();

        for name in ["dcc-supervisor", "dcc-ctl", "dcc-exec"] {
            let path = rt.host_path.join(name);
            assert!(path.is_file(), "{name} should exist");
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.starts_with("#!/bin/sh\n"), "{name} shebang");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                let mode = std::fs::metadata(&path).unwrap().permissions().mode();
                assert!(mode & 0o111 != 0, "{name} should be executable");
            }
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
}
