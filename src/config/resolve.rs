use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::Context as _;

use crate::config::{merge::merge, parse_config_file, DevcontainerConfig, RawConfig};

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

    let extends_path = match &raw.extends {
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
/// Errors if `image` is absent. Defaults `container_user` to `"dev"`.
pub(crate) fn raw_to_config(raw: RawConfig, source: &Path) -> anyhow::Result<DevcontainerConfig> {
    let image = raw.image.ok_or_else(|| {
        anyhow::anyhow!(
            "no `image` specified in `{}` or any file it extends",
            source.display()
        )
    })?;
    Ok(DevcontainerConfig {
        image,
        features: raw.features.unwrap_or_default(),
        container_env: raw.container_env.unwrap_or_default(),
        container_user: raw.container_user.unwrap_or_else(|| "dev".to_string()),
        mounts: raw.mounts.unwrap_or_default(),
        forward_ports: raw.forward_ports.unwrap_or_default(),
        entrypoint: raw.entrypoint,
    })
}

#[cfg(test)]
mod tests {
    use crate::{cache::CacheDir, config::load_config, workspace::Workspace};
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
        }
    }

    fn stub_cache_dir() -> CacheDir {
        CacheDir {
            host_path: PathBuf::from("/tmp/.dcc/test"),
        }
    }

    #[test]
    fn test_simple_load() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "image": "rust:latest" }"#);
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.image, "rust:latest");
    }

    #[test]
    fn test_default_container_user() {
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
    fn test_missing_image_error() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "containerUser": "dev" }"#);
        let err = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap_err();
        assert!(
            err.to_string().contains("image"),
            "expected error to mention 'image', got: {err}"
        );
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
        assert_eq!(config.image, "ubuntu:22.04");
        assert_eq!(config.container_user, "myuser");
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
        assert_eq!(config.image, "alpine:3");
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
        assert_eq!(config.image, "child-image:2");
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
        assert_eq!(config.image, "base-image:1");
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
    fn test_entrypoint_child_replaces_parent() {
        let dir = TempDir::new().unwrap();
        write(
            dir.path(),
            "base.json",
            r#"{ "image": "x:1", "entrypoint": ["bash"] }"#,
        );
        let child = write(
            dir.path(),
            "child.json",
            r#"{ "extends": "base.json", "entrypoint": ["zsh"] }"#,
        );
        let config = load_config(&child, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert_eq!(config.entrypoint.as_deref(), Some(&["zsh".to_string()][..]));
    }

    #[test]
    fn test_empty_features_is_empty_map() {
        let dir = TempDir::new().unwrap();
        let path = write(dir.path(), "dev.json", r#"{ "image": "x:1" }"#);
        let config = load_config(&path, &stub_workspace(), &stub_cache_dir(), false).unwrap();
        assert!(config.features.is_empty());
    }
}
