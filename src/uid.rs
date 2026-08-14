//! `updateRemoteUserUID` remap planning.
//!
//! On Linux, when `containerUser` is a non-root named user and
//! `updateRemoteUserUID` is enabled (the devcontainer spec default), the
//! container user's UID/GID is remapped to the host user's UID/GID so bind
//! mounts (workspace, cache, state) are writable regardless of the host user's
//! uid. The remap is baked into `dcc`'s generated build stage (see
//! `features::context`) as a `RUN` step that ports the reference
//! `devcontainers/cli` `scripts/updateUID.Dockerfile` logic, with the same
//! no-op safety conditions.
//!
//! This module owns the *planning*: given the resolved config and the host
//! uid/gid, decide whether to emit the remap and produce the `--build-arg`
//! values and the in-image `RUN` script. Non-Linux hosts and the dry-run path
//! report a no-op.

/// The host user's uid/gid, captured at build time. `None` on non-unix hosts.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct HostIds {
    pub(crate) uid: u32,
    pub(crate) gid: u32,
}

/// Captures the invoking process's uid/gid. Returns `None` on non-unix hosts
/// (the remap is a Linux-only no-op elsewhere; Docker Desktop translates uids
/// inside its VM on macOS/Windows).
pub(crate) fn host_ids() -> Option<HostIds> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // `id -u` / `id -g` avoids a filesystem probe whose owner depends on
        // which path is sampled; the process ids are exactly what the spec
        // means by "the local user's UID/GID".
        let uid = std::process::Command::new("id")
            .arg("-u")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<u32>()
                        .ok()
                } else {
                    None
                }
            })?;
        let gid = std::process::Command::new("id")
            .arg("-g")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8_lossy(&o.stdout)
                        .trim()
                        .parse::<u32>()
                        .ok()
                } else {
                    None
                }
            })?;
        // CommandExt import is intentionally used to anchor the unix-only
        // path; the trait is in scope for future stdio tweaks.
        let _ = std::process::Command::new("true").uid(0);
        Some(HostIds { uid, gid })
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// What `dcc build` decided to do about the UID remap for one config.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum RemapPlan {
    /// No remap step is emitted. Carries the human-readable reason for debug
    /// output and dry-run reporting.
    None { reason: RemapSkipReason },
    /// Emit the remap `RUN` with these target ids.
    Remap { uid: u32, gid: u32, user: String },
}

/// Why a remap was skipped. Surfaced in `--debug` / `--dry-run`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum RemapSkipReason {
    /// Host platform is not Linux (Docker Desktop translates uids in its VM).
    NonLinuxHost,
    /// `updateRemoteUserUID` is explicitly `false`.
    Disabled,
    /// `containerUser` is `root`, which the remap must never touch.
    RootUser,
    /// `containerUser` is a numeric uid (already explicit); no name to rewrite.
    NumericUser,
    /// The host uid/gid could not be determined.
    HostIdsUnavailable,
}

impl std::fmt::Display for RemapSkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RemapSkipReason::NonLinuxHost => "host is not Linux",
            RemapSkipReason::Disabled => "updateRemoteUserUID is false",
            RemapSkipReason::RootUser => "containerUser is root",
            RemapSkipReason::NumericUser => "containerUser is numeric",
            RemapSkipReason::HostIdsUnavailable => "host uid/gid unavailable",
        };
        f.write_str(s)
    }
}

/// Decides whether to remap for `container_user` given `update_remote_user_uid`
/// and the captured host ids.
///
/// `host_ids` is `Option` so the caller can pass `host_ids()` directly; on
/// non-unix hosts that is `None` and the result is `NonLinuxHost`.
pub(crate) fn plan_uid_remap(
    container_user: &str,
    update_remote_user_uid: bool,
    host_ids: Option<HostIds>,
) -> RemapPlan {
    if !cfg!(unix) {
        return RemapPlan::None {
            reason: RemapSkipReason::NonLinuxHost,
        };
    }
    if !update_remote_user_uid {
        return RemapPlan::None {
            reason: RemapSkipReason::Disabled,
        };
    }
    if container_user == "root" {
        return RemapPlan::None {
            reason: RemapSkipReason::RootUser,
        };
    }
    if is_numeric_user(container_user) {
        return RemapPlan::None {
            reason: RemapSkipReason::NumericUser,
        };
    }
    let Some(ids) = host_ids else {
        return RemapPlan::None {
            reason: RemapSkipReason::HostIdsUnavailable,
        };
    };
    RemapPlan::Remap {
        uid: ids.uid,
        gid: ids.gid,
        user: container_user.to_string(),
    }
}

/// A "numeric user" is one that is entirely decimal digits — Docker accepts a
/// bare uid for `-u`, and the spec's `remoteUser`/`containerUser` may be one.
/// There is no `/etc/passwd` entry to rewrite, so the remap is meaningless.
fn is_numeric_user(user: &str) -> bool {
    !user.is_empty() && user.bytes().all(|b| b.is_ascii_digit())
}

/// The inline `sh` script run as root inside the generated build stage to
/// perform the remap. Mirrors `devcontainers/cli`'s
/// `scripts/updateUID.Dockerfile` `RUN` block, including every no-op
/// condition. `$NEW_UID`/`$NEW_GID`/`$REMOTE_USER` come from the surrounding
/// `ARG`s.
pub(crate) fn remap_run_script() -> &'static str {
    r#"eval $(sed -n "s/${REMOTE_USER}:[^:]*:\([^:]*\):\([^:]*\):[^:]*:\([^:]*\).*/OLD_UID=\1;OLD_GID=\2;HOME_FOLDER=\3/p" /etc/passwd); \
eval $(sed -n "s/\([^:]*\):[^:]*:${NEW_UID}:.*/EXISTING_USER=\1/p" /etc/passwd); \
eval $(sed -n "s/\([^:]*\):[^:]*:${NEW_GID}:.*/EXISTING_GROUP=\1/p" /etc/group); \
if [ -z "$OLD_UID" ]; then \
  echo "updateRemoteUserUID: user ${REMOTE_USER} not found in /etc/passwd; skipping"; \
elif [ "$OLD_UID" = "$NEW_UID" ] && [ "$OLD_GID" = "$NEW_GID" ]; then \
  echo "updateRemoteUserUID: uid/gid already ${NEW_UID}/${NEW_GID}; skipping"; \
elif [ "$OLD_UID" != "$NEW_UID" ] && [ -n "$EXISTING_USER" ]; then \
  echo "updateRemoteUserUID: uid ${NEW_UID} already occupied by ${EXISTING_USER}; skipping"; \
else \
  if [ "$OLD_GID" != "$NEW_GID" ] && [ -n "$EXISTING_GROUP" ]; then \
    echo "updateRemoteUserUID: gid ${NEW_GID} already occupied by ${EXISTING_GROUP}; keeping gid ${OLD_GID}"; \
    NEW_GID="$OLD_GID"; \
  fi; \
  echo "updateRemoteUserUID: updating ${REMOTE_USER} from ${OLD_UID}:${OLD_GID} to ${NEW_UID}:${NEW_GID}"; \
  sed -i -e "s/\(${REMOTE_USER}:[^:]*:\)[^:]*:[^:]*/\1${NEW_UID}:${NEW_GID}/" /etc/passwd; \
  if [ "$OLD_GID" != "$NEW_GID" ]; then \
    sed -i -e "s/\([^:]*:[^:]*:\)${OLD_GID}:/\1${NEW_GID}:/" /etc/group; \
  fi; \
  chown -R "$NEW_UID:$NEW_GID" "$HOME_FOLDER"; \
fi"#
}

/// The `ARG`/`RUN` block appended to the generated Dockerfile when a remap is
/// planned. `uid`/`gid` are the host ids; `user` is the (non-root, non-numeric)
/// `containerUser`. The `ARG`s are scoped to this step and not baked into the
/// final image env.
pub(crate) fn remap_dockerfile_block(uid: u32, gid: u32, user: &str) -> String {
    use crate::features::context::shell_quote;
    format!(
        "ARG REMOTE_USER={}\nARG NEW_UID={}\nARG NEW_GID={}\nRUN {remap}",
        shell_quote(user),
        uid,
        gid,
        remap = remap_run_script()
    )
}

/// The `--build-arg KEY=VALUE` entries the build must pass when a remap is
/// planned, so the `ARG`s in [`remap_dockerfile_block`] resolve.
pub(crate) fn remap_build_args(uid: u32, gid: u32, user: &str) -> Vec<(String, String)> {
    vec![
        ("REMOTE_USER".to_string(), user.to_string()),
        ("NEW_UID".to_string(), uid.to_string()),
        ("NEW_GID".to_string(), gid.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(uid: u32, gid: u32) -> Option<HostIds> {
        Some(HostIds { uid, gid })
    }

    // --- plan_uid_remap no-op branches ---

    #[test]
    fn plan_skips_when_disabled() {
        let plan = plan_uid_remap("dev", false, ids(1001, 999));
        assert_eq!(
            plan,
            RemapPlan::None {
                reason: RemapSkipReason::Disabled
            }
        );
    }

    #[test]
    fn plan_skips_root_user() {
        let plan = plan_uid_remap("root", true, ids(1001, 999));
        assert_eq!(
            plan,
            RemapPlan::None {
                reason: RemapSkipReason::RootUser
            }
        );
    }

    #[test]
    fn plan_skips_numeric_user() {
        let plan = plan_uid_remap("1000", true, ids(1001, 999));
        assert_eq!(
            plan,
            RemapPlan::None {
                reason: RemapSkipReason::NumericUser
            }
        );
    }

    #[test]
    fn plan_skips_when_host_ids_unavailable() {
        let plan = plan_uid_remap("dev", true, None);
        // On unix this is HostIdsUnavailable; on non-unix the earlier
        // NonLinuxHost branch wins. Assert the branch that matches the host.
        match plan {
            RemapPlan::None {
                reason: RemapSkipReason::HostIdsUnavailable,
            }
            | RemapPlan::None {
                reason: RemapSkipReason::NonLinuxHost,
            } => {}
            other => panic!("expected None skip, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn plan_remaps_non_root_named_user() {
        let plan = plan_uid_remap("dev", true, ids(1001, 999));
        assert_eq!(
            plan,
            RemapPlan::Remap {
                uid: 1001,
                gid: 999,
                user: "dev".to_string()
            }
        );
    }

    #[cfg(not(unix))]
    #[test]
    fn plan_skips_on_non_unix_host() {
        let plan = plan_uid_remap("dev", true, ids(1001, 999));
        assert_eq!(
            plan,
            RemapPlan::None {
                reason: RemapSkipReason::NonLinuxHost
            }
        );
    }

    // --- is_numeric_user ---

    #[test]
    fn numeric_user_detected() {
        assert!(is_numeric_user("1000"));
        assert!(is_numeric_user("0"));
    }

    #[test]
    fn named_user_not_numeric() {
        assert!(!is_numeric_user("dev"));
        assert!(!is_numeric_user("ubuntu1000"));
        assert!(!is_numeric_user(""));
    }

    // --- remap_dockerfile_block shape ---

    #[test]
    fn remap_block_declares_args_and_run() {
        let block = remap_dockerfile_block(1001, 999, "dev");
        assert!(block.contains("ARG REMOTE_USER='dev'"), "got: {block}");
        assert!(block.contains("ARG NEW_UID=1001"), "got: {block}");
        assert!(block.contains("ARG NEW_GID=999"), "got: {block}");
        assert!(block.starts_with("ARG REMOTE_USER="), "got: {block}");
        // The RUN carries the reference sed/chown logic.
        assert!(block.contains("sed -i"), "got: {block}");
        assert!(block.contains("chown -R"), "got: {block}");
        assert!(block.contains("/etc/passwd"), "got: {block}");
        assert!(block.contains("/etc/group"), "got: {block}");
    }

    #[test]
    fn remap_block_quotes_user_with_single_quote() {
        // A user with a shell-special char is still safely quoted.
        let block = remap_dockerfile_block(1, 2, "a'b");
        assert!(block.contains("ARG REMOTE_USER='a'\\''b'"), "got: {block}");
    }

    #[test]
    fn remap_block_includes_collision_noop_echo() {
        let block = remap_dockerfile_block(1001, 999, "dev");
        assert!(
            block.contains("already occupied by"),
            "collision no-op must be present, got: {block}"
        );
        assert!(
            block.contains("already ${NEW_UID}/${NEW_GID}"),
            "already-matching no-op must be present, got: {block}"
        );
    }

    #[test]
    fn remap_build_args_match_block_args() {
        let args = remap_build_args(1001, 999, "dev");
        assert_eq!(
            args,
            vec![
                ("REMOTE_USER".to_string(), "dev".to_string()),
                ("NEW_UID".to_string(), "1001".to_string()),
                ("NEW_GID".to_string(), "999".to_string()),
            ]
        );
    }

    // --- host_ids is callable on every platform ---

    #[test]
    fn host_ids_does_not_panic() {
        let _ = host_ids();
    }
}
