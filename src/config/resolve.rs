use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::Context as _;

use crate::{
    config::{
        merge::merge, parse_config_file, vars, DevcontainerConfig, RawConfig, StateEntry,
        StateKind, UnsafeRuntimeConfig, DEFAULT_CONTAINER_USER,
    },
    lifecycle::LifecycleHooks,
};

/// Recursively load a RawConfig, following `extends` chains.
/// `visited` contains canonicalized paths already in the chain (for cycle detection).
pub(crate) fn load_raw(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    strict: bool,
) -> anyhow::Result<RawConfig> {
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("failed to resolve path `{}`", path.display()))?;

    if visited.contains(&canonical) {
        anyhow::bail!("`{}` closes a circular extends chain", canonical.display());
    }
    visited.insert(canonical);

    let raw = parse_config_file(path, strict)?;

    let extends_path = match raw
        .customizations
        .as_ref()
        .and_then(|c| c.dcc.as_ref())
        .and_then(|dcc| dcc.extends.as_ref())
    {
        None => return Ok(raw),
        Some(e) => {
            let parent_dir = path.parent().with_context(|| {
                format!(
                    "`{}` has an extends field but no parent directory",
                    path.display()
                )
            })?;
            parent_dir.join(e)
        }
    };

    let parent = load_raw(&extends_path, visited, strict).with_context(|| {
        format!(
            "failed to load parent config `{}` (extended from `{}`)",
            extends_path.display(),
            path.display()
        )
    })?;

    Ok(merge(parent, raw))
}

/// Convert a fully-merged RawConfig to DevcontainerConfig.
/// Errors if neither `image` nor official `build` is present, or if both are present.
pub(crate) fn raw_to_config(raw: RawConfig, source: &Path) -> anyhow::Result<DevcontainerConfig> {
    if raw.image.is_some() && raw.build.is_some() {
        anyhow::bail!(
            "`{}` sets both `image` and `build`; devcontainer profiles must choose exactly one image source",
            source.display()
        );
    }
    if raw.image.is_none() && raw.build.is_none() {
        anyhow::bail!(
            "no `image` or `build` specified in `{}` or any file it extends",
            source.display()
        );
    }
    Ok(DevcontainerConfig {
        name: raw.name,
        image: raw.image,
        build: raw.build,
        features: raw.features.unwrap_or_default(),
        container_env: raw.container_env.unwrap_or_default(),
        remote_env: raw.remote_env.unwrap_or_default(),
        container_user: raw
            .container_user
            .unwrap_or_else(|| DEFAULT_CONTAINER_USER.to_string()),
        mounts: raw.mounts.unwrap_or_default(),
        run_args: raw.run_args.unwrap_or_default(),
        unsafe_runtime: UnsafeRuntimeConfig {
            privileged: raw.privileged.unwrap_or(false),
            cap_add: raw.cap_add.unwrap_or_default(),
            security_opt: raw.security_opt.unwrap_or_default(),
        },
        forward_ports: raw.forward_ports.unwrap_or_default(),
        ports_attributes: raw.ports_attributes.unwrap_or_default(),
        other_ports_attributes: raw.other_ports_attributes,
        override_command: raw.override_command,
        update_remote_user_uid: raw.update_remote_user_uid.unwrap_or(true),
        workspace_folder: raw
            .workspace_folder
            .unwrap_or_else(|| vars::CONTAINER_WORKSPACE.to_string()),
        workspace_mount: raw.workspace_mount,
        initialize_command: raw.initialize_command,
        scripts: raw
            .customizations
            .as_ref()
            .and_then(|c| c.dcc.as_ref())
            .and_then(|dcc| dcc.commands.clone())
            .unwrap_or_default(),
        state: raw
            .customizations
            .and_then(|c| c.dcc)
            .and_then(|dcc| dcc.state)
            .unwrap_or_default(),
        lifecycle: LifecycleHooks {
            on_create_command: raw.on_create_command,
            update_content_command: raw.update_content_command,
            post_create_command: raw.post_create_command,
            post_start_command: raw.post_start_command,
            post_attach_command: raw.post_attach_command,
        },
    })
}

pub(crate) fn validate_state_entries(state: Vec<StateEntry>) -> anyhow::Result<Vec<StateEntry>> {
    validate_state_entries_inner(state, DeferredContainerEnv::Reject)
}

pub(crate) fn validate_state_entries_allowing_deferred_container_env(
    state: Vec<StateEntry>,
) -> anyhow::Result<Vec<StateEntry>> {
    validate_state_entries_inner(state, DeferredContainerEnv::Allow)
}

pub(crate) fn resolve_state_entries_container_env(
    state: &[StateEntry],
    env: &std::collections::HashMap<String, String>,
) -> anyhow::Result<Vec<StateEntry>> {
    let resolved = state
        .iter()
        .map(|entry| {
            let path = vars::resolve_container_env(&entry.path, env)
                .with_context(|| format!("in state path `{}`", entry.path))?;
            anyhow::Ok(StateEntry {
                path,
                kind: entry.kind,
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    validate_state_entries(resolved)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DeferredContainerEnv {
    Allow,
    Reject,
}

fn validate_state_entries_inner(
    state: Vec<StateEntry>,
    deferred: DeferredContainerEnv,
) -> anyhow::Result<Vec<StateEntry>> {
    let mut validated = Vec::new();
    for entry in state {
        if has_deferred_container_env(&entry.path) {
            validate_deferred_state_entry(&entry, deferred)?;
            insert_state_entry(&mut validated, entry)?;
            continue;
        }

        let path = normalize_state_path(&entry.path)?;
        insert_state_entry(
            &mut validated,
            StateEntry {
                path,
                kind: entry.kind,
            },
        )?;
    }
    Ok(validated)
}

fn has_deferred_container_env(path: &str) -> bool {
    vars::unresolved_variables(path)
        .iter()
        .any(|token| token.starts_with("${containerEnv:"))
}

fn validate_deferred_state_entry(
    entry: &StateEntry,
    deferred: DeferredContainerEnv,
) -> anyhow::Result<()> {
    let unresolved = vars::unresolved_variables(&entry.path);
    let unsupported: Vec<&str> = unresolved
        .iter()
        .map(String::as_str)
        .filter(|token| !token.starts_with("${containerEnv:"))
        .collect();
    if !unsupported.is_empty() {
        anyhow::bail!(
            "customizations.dcc.state path `{}` contains unresolved variable(s) {}",
            entry.path,
            unsupported.join(", ")
        );
    }
    if deferred == DeferredContainerEnv::Reject {
        anyhow::bail!(
            "customizations.dcc.state path `{}` contains unresolved variable(s) {}",
            entry.path,
            unresolved.join(", ")
        );
    }
    if entry.path.is_empty() {
        anyhow::bail!("customizations.dcc.state path is empty");
    }
    Ok(())
}

/// Tier 1: the whole path and its subtree are blocked. Masking any of these can
/// only break the container or `dcc` itself; there is no legitimate state target
/// beneath them.
const RESERVED_SUBTREE_PATHS: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/tmp",
    "/run",
    "/var/run",
    "/var/lock",
    "/boot",
    "/bin",
    "/sbin",
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/usr/bin",
    "/usr/sbin",
    "/usr/lib",
    "/usr/lib32",
    "/usr/lib64",
    "/usr/libx32",
    "/etc",
    "/workspace/.dcc",
    "/cache",
    "/usr/local/share/dcc",
];

/// Tier 2: only the exact path is blocked; specific subdirectories stay valid.
/// Each entry pairs the blocked path with a one-line rationale used in the error.
const RESERVED_EXACT_PATHS: &[(&str, &str)] = &[
    (
        "/usr",
        "masks the entire system tree; declare a specific subdirectory such as `/usr/local/cargo`",
    ),
    (
        "/var",
        "masks all system state; declare a specific subdirectory such as `/var/cache/apt`",
    ),
    (
        "/home",
        "masks every user's home; declare a specific subdirectory such as `/home/dev/.cargo`",
    ),
    (
        "/root",
        "masks root's entire home; declare a specific subdirectory such as `/root/.cargo`",
    ),
    (
        "/opt",
        "masks all opt trees; declare a specific subdirectory such as `/opt/toolchain/cache`",
    ),
    (
        "/workspace",
        "shadows the repository bind mount; declare a specific subdirectory such as `/workspace/target`",
    ),
    ("/srv", "is a low-value bare state target; declare a specific subdirectory"),
    ("/mnt", "is a low-value bare state target; declare a specific subdirectory"),
    ("/media", "is a low-value bare state target; declare a specific subdirectory"),
];

fn normalize_state_path(path: &str) -> anyhow::Result<String> {
    let unresolved = vars::unresolved_variables(path);
    if !unresolved.is_empty() {
        anyhow::bail!(
            "customizations.dcc.state path `{path}` contains unresolved variable(s) {}",
            unresolved.join(", ")
        );
    }
    if !path.starts_with('/') {
        anyhow::bail!("customizations.dcc.state path `{path}` must be absolute");
    }
    if path.contains('\0') {
        anyhow::bail!("customizations.dcc.state path `{path}` contains a NUL byte");
    }

    let mut segments = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment.contains("..") {
            anyhow::bail!(
                "customizations.dcc.state path `{path}` contains invalid segment `{segment}`"
            );
        }
        segments.push(segment);
    }

    if segments.is_empty() {
        anyhow::bail!("customizations.dcc.state path `/` is not allowed");
    }

    let normalized = format!("/{}", segments.join("/"));
    check_reserved_state_path(&normalized)?;
    Ok(normalized)
}

/// Rejects `normalized` when it targets a reserved container path. Subtree
/// reservations block the path and its descendants; exact reservations block only
/// the bare path. Both error shapes name the rejected path, the matched reserved
/// path, and a supported alternative so the message is self-diagnosing.
fn check_reserved_state_path(normalized: &str) -> anyhow::Result<()> {
    for reserved in RESERVED_SUBTREE_PATHS {
        if is_path_or_child(normalized, reserved) {
            anyhow::bail!(
                "customizations.dcc.state path `{normalized}` targets reserved system path `{reserved}`; use a lifecycle hook to manage system files"
            );
        }
    }
    for (reserved, reason) in RESERVED_EXACT_PATHS {
        if normalized == *reserved {
            anyhow::bail!("customizations.dcc.state path `{normalized}` {reason}");
        }
    }
    Ok(())
}

fn insert_state_entry(validated: &mut Vec<StateEntry>, entry: StateEntry) -> anyhow::Result<()> {
    for existing in validated.iter() {
        if existing.path == entry.path {
            if existing.kind == entry.kind {
                return Ok(());
            }
            anyhow::bail!(
                "customizations.dcc.state path `{}` is declared as both {} and {}",
                entry.path,
                state_kind_name(existing.kind),
                state_kind_name(entry.kind)
            );
        }
        if is_path_or_child(&entry.path, &existing.path)
            || is_path_or_child(&existing.path, &entry.path)
        {
            anyhow::bail!(
                "customizations.dcc.state paths `{}` and `{}` overlap; parent/child state paths are not allowed",
                existing.path,
                entry.path
            );
        }
    }
    validated.push(entry);
    Ok(())
}

fn is_path_or_child(path: &str, parent: &str) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn state_kind_name(kind: StateKind) -> &'static str {
    match kind {
        StateKind::Directory => "directory",
        StateKind::File => "file",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_state_entries_container_env, validate_state_entries,
        validate_state_entries_allowing_deferred_container_env,
    };
    use crate::{
        cache::CacheDir, config::load_config, lifecycle::LifecycleCommand, workspace::Workspace,
    };
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn write(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    fn stub_workspace() -> Workspace {
        Workspace {
            root: PathBuf::from("/tmp"),
            identity: "/tmp".to_string(),
        }
    }

    fn stub_cache_dir() -> CacheDir {
        CacheDir {
            host_path: PathBuf::from("/tmp/.dcc/test"),
            profile_name: crate::profile::ProfileName::new("test"),
        }
    }

    #[test]
    fn test_simple_load() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "image": "rust:latest" }"#);
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image.as_deref(), Some("rust:latest"));
    }

    #[test]
    fn test_lifecycle_commands_substituted_except_unsupported_initialize() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "initializeCommand": ["echo", "${localCacheFolder}"],
                "postCreateCommand": "echo ${localWorkspaceFolder}"
            }"#,
        );
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(
            config.initialize_command,
            Some(LifecycleCommand::Exec(vec![
                "echo".to_string(),
                "${localCacheFolder}".to_string(),
            ]))
        );
        assert_eq!(
            config.lifecycle.post_create_command,
            Some(LifecycleCommand::Shell("echo /tmp".to_string()))
        );
    }

    #[test]
    fn test_no_container_user_defaults_to_dev() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "image": "rust:latest" }"#);
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.container_user, "dev");
    }

    #[test]
    fn test_explicit_container_user() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{ "image": "rust:latest", "containerUser": "root" }"#,
        );
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.container_user, "root");
    }

    #[test]
    fn test_update_remote_user_uid_defaults_to_true() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "image": "rust:latest" }"#);
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(
            config.update_remote_user_uid,
            "updateRemoteUserUID must default to true"
        );
    }

    #[test]
    fn test_update_remote_user_uid_false_respected() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{ "image": "rust:latest", "updateRemoteUserUID": false }"#,
        );
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(
            !config.update_remote_user_uid,
            "explicit updateRemoteUserUID false must be honored"
        );
    }

    #[test]
    fn test_update_remote_user_uid_not_in_extra() {
        // The property is recognized, so it must not be collected into `extra`
        // (which would warn/default-reject it).
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{ "image": "rust:latest", "updateRemoteUserUID": true }"#,
        );
        // Load in strict mode: an unrecognized field would error here.
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), true).unwrap();
        assert!(config.update_remote_user_uid);
    }

    #[test]
    fn test_update_remote_user_uid_child_overrides_parent() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "base.json",
            r#"{ "image": "ubuntu:22.04", "updateRemoteUserUID": false }"#,
        );
        let child = write(
            dir.path(),
            "child.json",
            r#"{ "extends": "base.json", "updateRemoteUserUID": true }"#,
        );
        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(config.update_remote_user_uid);

        // And the reverse: child false overrides parent true.
        write(
            dir.path(),
            "base2.json",
            r#"{ "image": "ubuntu:22.04", "updateRemoteUserUID": true }"#,
        );
        let child2 = write(
            dir.path(),
            "child2.json",
            r#"{ "extends": "base2.json", "updateRemoteUserUID": false }"#,
        );
        let config2 = load_config(&child2, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(!config2.update_remote_user_uid);
    }

    #[test]
    fn test_missing_image_error() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "containerUser": "dev" }"#);
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        assert!(
            err.to_string().contains("image") && err.to_string().contains("build"),
            "expected error to mention image/build, got: {err}"
        );
    }

    #[test]
    fn test_build_source_without_image() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{ "build": { "context": "..", "dockerfile": "Dockerfile" } }"#,
        );
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(config.image.is_none());
        let build = config.build.expect("build source should be present");
        assert_eq!(build.context, "..");
        assert_eq!(build.dockerfile, "Dockerfile");
    }

    #[test]
    fn test_image_and_build_conflict() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{ "image": "rust:1", "build": { "dockerfile": "Dockerfile" } }"#,
        );
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("both `image` and `build`"), "got: {msg}");
    }

    #[test]
    fn test_two_file_extends() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "base.json", r#"{ "image": "ubuntu:22.04" }"#);
        let child = write(
            dir.path(),
            "child.json",
            r#"{ "extends": "base.json", "containerUser": "myuser" }"#,
        );
        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image.as_deref(), Some("ubuntu:22.04"));
        assert_eq!(config.container_user, "myuser");
    }

    #[test]
    fn test_two_file_customizations_dcc_extends() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "base.json",
            r#"{
                "image": "ubuntu:22.04",
                "customizations": {
                    "dcc": { "commands": { "test": "cargo test" } }
                }
            }"#,
        );
        let child = write(
            dir.path(),
            "child.json",
            r#"{
                "customizations": {
                    "dcc": {
                        "extends": "base.json",
                        "commands": { "build": "cargo build" },
                        "state": ["/home/dev/.cache"]
                    }
                },
                "containerUser": "myuser"
            }"#,
        );
        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image.as_deref(), Some("ubuntu:22.04"));
        assert_eq!(config.container_user, "myuser");
        assert_eq!(
            config.scripts.get("test").map(String::as_str),
            Some("cargo test")
        );
        assert_eq!(
            config.scripts.get("build").map(String::as_str),
            Some("cargo build")
        );
        assert_eq!(config.state.len(), 1);
        assert_eq!(config.state[0].path, "/home/dev/.cache");
    }

    #[test]
    fn state_paths_are_substituted_normalized_and_deduplicated() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "customizations": {
                    "dcc": {
                        "state": [
                            "${containerWorkspaceFolder}/target/",
                            "/workspace//target"
                        ]
                    }
                }
            }"#,
        );
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(
            config.state,
            vec![crate::config::StateEntry {
                path: "/workspace/target".to_string(),
                kind: crate::config::StateKind::Directory,
            }]
        );
    }

    #[test]
    fn state_rejects_relative_path() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "customizations": { "dcc": { "state": ["relative/cache"] } }
            }"#,
        );
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(full.contains("absolute"), "got: {full}");
    }

    #[test]
    fn state_rejects_root_path() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "customizations": { "dcc": { "state": ["/"] } }
            }"#,
        );
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(full.contains("not allowed"), "got: {full}");
    }

    #[test]
    fn state_rejects_dotdot_segments() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "customizations": { "dcc": { "state": ["/home/dev/../cache"] } }
            }"#,
        );
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(full.contains("invalid segment"), "got: {full}");
    }

    #[test]
    fn state_rejects_unresolved_unsupported_variables() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "customizations": { "dcc": { "state": ["${unknown}/cache"] } }
            }"#,
        );
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(full.contains("${unknown}"), "got: {full}");
    }

    #[test]
    fn state_rejects_local_host_variables() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "customizations": {
                    "dcc": { "state": ["${localCacheFolder}/cargo"] }
                }
            }"#,
        );
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(full.contains("${localCacheFolder}"), "got: {full}");
    }

    #[test]
    fn state_rejects_reserved_runtime_paths() {
        for reserved in [
            "/tmp/cache",
            "/run/app",
            "/proc/self",
            "/sys/fs",
            "/dev/shm",
        ] {
            let err = validate_state_entries(vec![crate::config::StateEntry {
                path: reserved.to_string(),
                kind: crate::config::StateKind::Directory,
            }])
            .unwrap_err();
            assert!(
                err.to_string().contains("reserved"),
                "expected reserved-path error for {reserved}, got: {err:#}"
            );
        }
    }

    #[test]
    fn state_rejects_reserved_workspace_dcc_path() {
        let err = validate_state_entries(vec![crate::config::StateEntry {
            path: "/workspace/.dcc/state".to_string(),
            kind: crate::config::StateKind::Directory,
        }])
        .unwrap_err();
        assert!(err.to_string().contains("/workspace/.dcc"));
    }

    // ── T-0021: critical container path guards ───────────────────────────────

    fn reject_dir(path: &str) -> anyhow::Error {
        validate_state_entries(vec![crate::config::StateEntry {
            path: path.to_string(),
            kind: crate::config::StateKind::Directory,
        }])
        .expect_err("expected rejection")
    }

    fn accept_dir(path: &str) -> Vec<crate::config::StateEntry> {
        validate_state_entries(vec![crate::config::StateEntry {
            path: path.to_string(),
            kind: crate::config::StateKind::Directory,
        }])
        .expect("expected acceptance")
    }

    #[test]
    fn state_tier1_rejects_exact_and_child_for_every_path() {
        for reserved in [
            "/proc",
            "/sys",
            "/dev",
            "/tmp",
            "/run",
            "/var/run",
            "/var/lock",
            "/boot",
            "/bin",
            "/sbin",
            "/lib",
            "/lib32",
            "/lib64",
            "/libx32",
            "/usr/bin",
            "/usr/sbin",
            "/usr/lib",
            "/usr/lib32",
            "/usr/lib64",
            "/usr/libx32",
            "/etc",
            "/workspace/.dcc",
            "/cache",
            "/usr/local/share/dcc",
        ] {
            let err = reject_dir(reserved);
            assert!(
                err.to_string().contains(reserved),
                "exact {reserved} should be rejected naming the path, got: {err:#}"
            );
            let child = format!("{reserved}/sub");
            let err = reject_dir(&child);
            assert!(
                err.to_string().contains(reserved),
                "child {child} should be rejected naming `{reserved}`, got: {err:#}"
            );
        }
    }

    #[test]
    fn state_tier1_error_names_alternative() {
        let err = reject_dir("/etc/passwd");
        let msg = err.to_string();
        assert!(msg.contains("/etc"), "got: {msg}");
        assert!(msg.contains("lifecycle hook"), "got: {msg}");
    }

    #[test]
    fn state_rejects_cache_and_children() {
        for path in ["/cache", "/cache/state", "/cache/runtime/anything"] {
            let err = reject_dir(path);
            assert!(
                err.to_string().contains("/cache"),
                "{path} should be rejected naming `/cache`, got: {err:#}"
            );
        }
    }

    #[test]
    fn state_rejects_dcc_assets_and_children() {
        for path in [
            "/usr/local/share/dcc",
            "/usr/local/share/dcc/bin",
            "/usr/local/share/dcc/dcc-supervisor",
        ] {
            let err = reject_dir(path);
            assert!(
                err.to_string().contains("/usr/local/share/dcc"),
                "{path} should be rejected naming the dcc asset path, got: {err:#}"
            );
        }
    }

    #[test]
    fn state_rejects_both_bin_spellings() {
        // merged-usr: /bin -> /usr/bin; a textual guard must list both spellings.
        for path in ["/bin/sh", "/usr/bin/sh"] {
            let err = reject_dir(path);
            assert!(
                err.to_string().contains("/bin") || err.to_string().contains("/usr/bin"),
                "{path} should be rejected, got: {err:#}"
            );
        }
    }

    #[test]
    fn state_tier2_rejects_exact_but_accepts_child() {
        for (reserved, child) in [
            ("/usr", "/usr/local/cargo"),
            ("/var", "/var/cache/apt"),
            ("/home", "/home/dev/.cargo"),
            ("/root", "/root/.cargo"),
            ("/opt", "/opt/toolchain/cache"),
            ("/workspace", "/workspace/target"),
            ("/srv", "/srv/app"),
            ("/mnt", "/mnt/data"),
            ("/media", "/media/usb"),
        ] {
            let err = reject_dir(reserved);
            assert!(
                err.to_string().contains(reserved),
                "exact {reserved} should be rejected, got: {err:#}"
            );
            let accepted = accept_dir(child);
            assert_eq!(accepted[0].path, child, "{child} should be accepted");
        }
    }

    #[test]
    fn state_tier2_error_names_alternative() {
        let err = reject_dir("/home");
        let msg = err.to_string();
        assert!(msg.contains("/home/dev/.cargo"), "got: {msg}");
    }

    #[test]
    fn state_accepts_known_legitimate_nested_paths() {
        for path in [
            "/usr/local/cargo",
            "/var/cache/apt",
            "/home/dev/.cargo",
            "/root/.cargo",
            "/workspace/target",
        ] {
            let accepted = accept_dir(path);
            assert_eq!(accepted[0].path, path, "{path} should be accepted");
        }
    }

    #[test]
    fn state_rejects_critical_path_after_container_env_resolution() {
        // A deferred path that only becomes critical after ${containerEnv:VAR}
        // resolution must be rejected at the resolution point.
        let deferred = vec![crate::config::StateEntry {
            path: "${containerEnv:HOME}".to_string(),
            kind: crate::config::StateKind::Directory,
        }];
        // First: deferred path passes config-load validation.
        let loaded = validate_state_entries_allowing_deferred_container_env(deferred.clone())
            .expect("deferred path should pass load-time validation");
        assert_eq!(loaded[0].path, "${containerEnv:HOME}");
        // Then: resolving HOME=/etc is rejected at the resolution point.
        let env = std::collections::HashMap::from([("HOME".to_string(), "/etc".to_string())]);
        let err = resolve_state_entries_container_env(&loaded, &env).unwrap_err();
        assert!(
            err.to_string().contains("/etc"),
            "resolved /etc should be rejected, got: {err:#}"
        );
    }

    #[test]
    fn state_accepts_nested_path_after_container_env_resolution() {
        let deferred = vec![crate::config::StateEntry {
            path: "${containerEnv:HOME}/.cargo".to_string(),
            kind: crate::config::StateKind::Directory,
        }];
        let loaded = validate_state_entries_allowing_deferred_container_env(deferred).unwrap();
        let env = std::collections::HashMap::from([("HOME".to_string(), "/home/dev".to_string())]);
        let resolved = resolve_state_entries_container_env(&loaded, &env).unwrap();
        assert_eq!(resolved[0].path, "/home/dev/.cargo");
    }

    #[test]
    fn state_rejects_file_kind_critical_path() {
        // File-kind state on a reserved path is still rejected (empty-file masking
        // of e.g. /etc/passwd is the motivating defect).
        let err = validate_state_entries(vec![crate::config::StateEntry {
            path: "/etc/passwd".to_string(),
            kind: crate::config::StateKind::File,
        }])
        .unwrap_err();
        assert!(err.to_string().contains("/etc"), "got: {err:#}");
    }

    #[test]
    fn state_rejects_conflicting_duplicate_kinds() {
        let err = validate_state_entries(vec![
            crate::config::StateEntry {
                path: "/home/dev/.tool".to_string(),
                kind: crate::config::StateKind::Directory,
            },
            crate::config::StateEntry {
                path: "/home/dev//.tool/".to_string(),
                kind: crate::config::StateKind::File,
            },
        ])
        .unwrap_err();
        assert!(
            err.to_string().contains("both directory and file"),
            "got: {err:#}"
        );
    }

    #[test]
    fn state_rejects_parent_child_overlap() {
        let err = validate_state_entries(vec![
            crate::config::StateEntry {
                path: "/home/dev/.cache".to_string(),
                kind: crate::config::StateKind::Directory,
            },
            crate::config::StateEntry {
                path: "/home/dev/.cache/tool".to_string(),
                kind: crate::config::StateKind::Directory,
            },
        ])
        .unwrap_err();
        assert!(err.to_string().contains("overlap"), "got: {err:#}");
    }

    #[test]
    fn state_defers_container_env_until_runtime_validation() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{
                "image": "rust:latest",
                "customizations": {
                    "dcc": {
                        "state": [{ "path": "${containerEnv:HOME}/.npmrc", "type": "file" }]
                    }
                }
            }"#,
        );
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.state[0].path, "${containerEnv:HOME}/.npmrc");

        let env = std::collections::HashMap::from([("HOME".to_string(), "/home/dev".to_string())]);
        let state = resolve_state_entries_container_env(&config.state, &env).unwrap();
        assert_eq!(
            state,
            vec![crate::config::StateEntry {
                path: "/home/dev/.npmrc".to_string(),
                kind: crate::config::StateKind::File,
            }]
        );
    }

    #[test]
    fn test_three_file_chain() {
        let dir = TempDir::new().unwrap();
        // C has image, B has env, A has feature
        write(dir.path(), "c.json", r#"{ "image": "alpine:3" }"#);
        write(
            dir.path(),
            "b.json",
            r#"{ "extends": "c.json", "containerEnv": { "MY_VAR": "hello" } }"#,
        );
        let a = write(
            dir.path(),
            "a.json",
            r#"{ "extends": "b.json", "features": { "ghcr.io/foo/bar:1": {} } }"#,
        );
        let config = load_config(&a, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image.as_deref(), Some("alpine:3"));
        assert_eq!(
            config.container_env.get("MY_VAR").map(|s| s.as_str()),
            Some("hello")
        );
        assert!(config.features.contains_key("ghcr.io/foo/bar:1"));
    }

    #[test]
    fn test_child_image_overrides_parent() {
        let dir = TempDir::new().unwrap();
        write(dir.path(), "base.json", r#"{ "image": "parent-image:1" }"#);
        let child = write(
            dir.path(),
            "child.json",
            r#"{ "extends": "base.json", "image": "child-image:2" }"#,
        );
        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image.as_deref(), Some("child-image:2"));
    }

    #[test]
    fn test_circular_two_files() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "a.json",
            r#"{ "extends": "b.json", "image": "x:1" }"#,
        );
        write(
            dir.path(),
            "b.json",
            r#"{ "extends": "a.json", "image": "y:1" }"#,
        );
        let a = dir.path().join("a.json");
        let err = load_config(&a, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(
            full.contains("circular"),
            "expected error chain to mention 'circular', got: {full}"
        );
    }

    #[test]
    fn test_circular_three_files() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "a.json",
            r#"{ "extends": "b.json", "image": "x:1" }"#,
        );
        write(dir.path(), "b.json", r#"{ "extends": "c.json" }"#);
        write(dir.path(), "c.json", r#"{ "extends": "a.json" }"#);
        let a = dir.path().join("a.json");
        let err = load_config(&a, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(
            full.contains("circular"),
            "expected error chain to mention 'circular', got: {full}"
        );
    }

    #[test]
    fn test_customizations_dcc_circular_extends() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "a.json",
            r#"{ "customizations": { "dcc": { "extends": "b.json" } }, "image": "x:1" }"#,
        );
        write(
            dir.path(),
            "b.json",
            r#"{ "customizations": { "dcc": { "extends": "a.json" } }, "image": "y:1" }"#,
        );
        let a = dir.path().join("a.json");
        let err = load_config(&a, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let full = format!("{err:#}");
        assert!(
            full.contains("circular"),
            "expected error chain to mention 'circular', got: {full}"
        );
    }

    #[test]
    fn test_missing_extends_target() {
        let dir = TempDir::new().unwrap();
        let path = write(
            dir.path(),
            "dev.json",
            r#"{ "extends": "nonexistent.json", "image": "x:1" }"#,
        );
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent.json") || msg.contains("nonexistent"),
            "expected error to mention the missing file, got: {msg}"
        );
    }

    #[test]
    fn test_extends_resolved_relative_to_file() {
        let dir = TempDir::new().unwrap();
        // Create .devcontainer/child.json and other/base.json
        let dc = dir.path().join(".devcontainer");
        let other = dir.path().join("other");
        std::fs::create_dir_all(&dc).unwrap();
        std::fs::create_dir_all(&other).unwrap();

        std::fs::write(other.join("base.json"), r#"{ "image": "base-image:1" }"#).unwrap();
        let child = write(&dc, "child.json", r#"{ "extends": "../other/base.json" }"#);

        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image.as_deref(), Some("base-image:1"));
    }

    #[test]
    fn test_customizations_dcc_extends_resolved_relative_to_file() {
        let dir = TempDir::new().unwrap();
        let dc = dir.path().join(".devcontainer");
        let other = dir.path().join("other");
        std::fs::create_dir_all(&dc).unwrap();
        std::fs::create_dir_all(&other).unwrap();

        std::fs::write(other.join("base.json"), r#"{ "image": "base-image:1" }"#).unwrap();
        let child = write(
            &dc,
            "child.json",
            r#"{ "customizations": { "dcc": { "extends": "../other/base.json" } } }"#,
        );

        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image.as_deref(), Some("base-image:1"));
    }

    #[test]
    fn test_features_merged() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "base.json",
            r#"{ "image": "x:1", "features": { "feat-a": {} } }"#,
        );
        let child = write(
            dir.path(),
            "child.json",
            r#"{ "extends": "base.json", "features": { "feat-b": {} } }"#,
        );
        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(
            config.features.contains_key("feat-a"),
            "feat-a should be present"
        );
        assert!(
            config.features.contains_key("feat-b"),
            "feat-b should be present"
        );
    }

    #[test]
    fn test_nested_parent_commands_legacy_child_scripts_child_wins() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "base.json",
            r#"{
                "image": "rust:1",
                "customizations": {
                    "dcc": {
                        "commands": {
                            "build": "make build",
                            "test": "make test"
                        }
                    }
                }
            }"#,
        );
        let child = write(
            dir.path(),
            "child.json",
            r#"{
                "extends": "base.json",
                "scripts": {
                    "build": "cargo build",
                    "lint": "cargo clippy"
                }
            }"#,
        );
        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(
            config.scripts.get("build").map(String::as_str),
            Some("cargo build")
        );
        assert_eq!(
            config.scripts.get("test").map(String::as_str),
            Some("make test")
        );
        assert_eq!(
            config.scripts.get("lint").map(String::as_str),
            Some("cargo clippy")
        );
    }

    #[test]
    fn test_empty_features_is_empty_map() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "image": "x:1" }"#);
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(config.features.is_empty());
    }
}
