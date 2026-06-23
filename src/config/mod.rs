use std::collections::HashMap;
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;
use serde::Deserialize;

use crate::{
    cache::CacheDir,
    lifecycle::{LifecycleCommand, LifecycleHooks},
    workspace::Workspace,
};

pub(crate) mod merge;
pub(crate) mod resolve;
pub(crate) mod vars;

/// The user `dcc build` runs feature install scripts as and `dcc run` passes to
/// `docker run -u` when `containerUser` is not set in the devcontainer config.
pub(crate) const DEFAULT_CONTAINER_USER: &str = "dev";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawConfig {
    pub(crate) extends: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) features: Option<IndexMap<String, serde_json::Value>>,
    pub(crate) container_env: Option<HashMap<String, String>>,
    pub(crate) remote_env: Option<HashMap<String, String>>,
    pub(crate) container_user: Option<String>,
    pub(crate) mounts: Option<Vec<String>>,
    pub(crate) forward_ports: Option<Vec<u16>>,
    pub(crate) initialize_command: Option<LifecycleCommand>,
    pub(crate) on_create_command: Option<LifecycleCommand>,
    pub(crate) update_content_command: Option<LifecycleCommand>,
    pub(crate) post_create_command: Option<LifecycleCommand>,
    pub(crate) post_start_command: Option<LifecycleCommand>,
    pub(crate) post_attach_command: Option<LifecycleCommand>,
    pub(crate) scripts: Option<HashMap<String, String>>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub(crate) struct DevcontainerConfig {
    pub(crate) name: Option<String>,
    pub(crate) image: String,
    pub(crate) features: IndexMap<String, serde_json::Value>,
    pub(crate) container_env: HashMap<String, String>,
    pub(crate) remote_env: HashMap<String, String>,
    pub(crate) container_user: String,
    pub(crate) mounts: Vec<String>,
    pub(crate) forward_ports: Vec<u16>,
    pub(crate) initialize_command: Option<LifecycleCommand>,
    pub(crate) lifecycle: LifecycleHooks,
    pub(crate) scripts: HashMap<String, String>,
}

pub(crate) fn parse_config_file(path: &Path, strict: bool) -> anyhow::Result<RawConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let raw: RawConfig = json5::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    check_extra_fields(&raw.extra, path, strict)?;
    Ok(raw)
}

fn check_extra_fields(
    extra: &HashMap<String, serde_json::Value>,
    path: &Path,
    strict: bool,
) -> anyhow::Result<()> {
    let mut keys: Vec<&str> = extra.keys().map(|s| s.as_str()).collect();
    keys.sort();
    for key in keys {
        if strict {
            anyhow::bail!("{}: unrecognized field '{}'", path.display(), key);
        } else {
            tracing::warn!(file = %path.display(), field = %key, "unrecognized devcontainer field");
        }
    }
    Ok(())
}

pub(crate) fn load_config(
    path: &Path,
    workspace: &Workspace,
    cache_dir: &CacheDir,
    strict: bool,
) -> anyhow::Result<DevcontainerConfig> {
    let mut visited = std::collections::HashSet::new();
    let raw = resolve::load_raw(path, &mut visited, strict)?;
    let config = resolve::raw_to_config(raw, path)?;
    Ok(vars::apply_substitutions(config, workspace, cache_dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::prelude::*;

    fn write_temp(contents: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), contents).unwrap();
        file
    }

    #[test]
    fn all_known_fields() {
        let file = write_temp(
            r#"{
                "extends": "base.json",
                "name": "example",
                "image": "rust:latest",
                "features": { "ghcr.io/devcontainers/features/node:1": { "version": "20" } },
                "containerEnv": { "FOO": "bar" },
                "remoteEnv": { "FOO": "bar" },
                "containerUser": "dev",
                "mounts": ["type=bind,src=/tmp,dst=/tmp"],
                "forwardPorts": [8080, 3000],
                "initializeCommand": "echo init",
                "onCreateCommand": ["echo", "create"],
                "updateContentCommand": "echo update",
                "postCreateCommand": "echo post-create",
                "postStartCommand": "echo post-start",
                "postAttachCommand": { "a": "echo a", "b": ["echo", "b"] },
                "scripts": { "build": "cargo build" }
            }"#,
        );
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.extends.as_deref(), Some("base.json"));
        assert_eq!(raw.name.as_deref(), Some("example"));
        assert_eq!(raw.image.as_deref(), Some("rust:latest"));
        assert!(raw.features.is_some());
        assert!(raw.container_env.is_some());
        assert!(raw
            .remote_env
            .as_ref()
            .is_some_and(|m| m.get("FOO").map(|s| s.as_str()) == Some("bar")));
        assert_eq!(raw.container_user.as_deref(), Some("dev"));
        assert_eq!(
            raw.mounts.as_deref(),
            Some(&[String::from("type=bind,src=/tmp,dst=/tmp")][..])
        );
        assert_eq!(raw.forward_ports.as_deref(), Some(&[8080u16, 3000u16][..]));
        assert_eq!(
            raw.initialize_command,
            Some(LifecycleCommand::Shell("echo init".to_string()))
        );
        assert_eq!(
            raw.on_create_command,
            Some(LifecycleCommand::Exec(vec![
                "echo".to_string(),
                "create".to_string()
            ]))
        );
        assert_eq!(
            raw.update_content_command,
            Some(LifecycleCommand::Shell("echo update".to_string()))
        );
        assert_eq!(
            raw.post_create_command,
            Some(LifecycleCommand::Shell("echo post-create".to_string()))
        );
        assert_eq!(
            raw.post_start_command,
            Some(LifecycleCommand::Shell("echo post-start".to_string()))
        );
        assert!(matches!(
            raw.post_attach_command,
            Some(LifecycleCommand::Parallel(_))
        ));
        assert!(raw.scripts.is_some());
        assert!(raw.extra.is_empty());
    }

    #[test]
    fn remote_env_parsed() {
        let file = write_temp(r#"{ "image": "rust:1", "remoteEnv": { "TOKEN": "abc" } }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        let remote_env = raw.remote_env.expect("remoteEnv should be Some");
        assert_eq!(remote_env.get("TOKEN").map(|s| s.as_str()), Some("abc"));
    }

    #[test]
    fn jsonc_trailing_comma() {
        let file = write_temp(r#"{ "image": "rust:1", }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.image.as_deref(), Some("rust:1"));
    }

    #[test]
    fn jsonc_line_comment() {
        let file = write_temp("// a comment\n{ \"image\": \"rust:1\" }");
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.image.as_deref(), Some("rust:1"));
    }

    #[test]
    fn unknown_field_warn_mode() {
        let file = write_temp(r#"{ "fooBarBaz": "foo" }"#);
        let result = parse_config_file(file.path(), false);
        assert!(result.is_ok());
        let raw = result.unwrap();
        assert!(raw.extra.contains_key("fooBarBaz"));
    }

    #[test]
    fn unknown_field_strict_mode() {
        let file = write_temp(r#"{ "fooBarBaz": "foo" }"#);
        let result = parse_config_file(file.path(), true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("fooBarBaz"));
    }

    #[test]
    fn name_is_known_in_strict_mode() {
        let file = write_temp(r#"{ "name": "example", "image": "rust:1" }"#);
        let raw = parse_config_file(file.path(), true).unwrap();
        assert_eq!(raw.name.as_deref(), Some("example"));
        assert!(raw.extra.is_empty());
    }

    #[test]
    fn empty_object() {
        let file = write_temp("{}");
        let raw = parse_config_file(file.path(), false).unwrap();
        assert!(raw.extends.is_none());
        assert!(raw.name.is_none());
        assert!(raw.image.is_none());
        assert!(raw.features.is_none());
        assert!(raw.container_env.is_none());
        assert!(raw.remote_env.is_none());
        assert!(raw.container_user.is_none());
        assert!(raw.mounts.is_none());
        assert!(raw.forward_ports.is_none());
        assert!(raw.initialize_command.is_none());
        assert!(raw.on_create_command.is_none());
        assert!(raw.update_content_command.is_none());
        assert!(raw.post_create_command.is_none());
        assert!(raw.post_start_command.is_none());
        assert!(raw.post_attach_command.is_none());
        assert!(raw.scripts.is_none());
        assert!(raw.extra.is_empty());
    }

    #[test]
    fn parse_error_contains_path() {
        let file = write_temp(r#"{ "image": }"#);
        let result = parse_config_file(file.path(), false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains(&file.path().display().to_string()));
    }

    #[test]
    fn camel_case_round_trip() {
        let file = write_temp(r#"{ "forwardPorts": [80, 5432] }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.forward_ports, Some(vec![80u16, 5432u16]));
    }

    #[test]
    fn features_uses_index_map_preserves_order() {
        let file = write_temp(r#"{ "features": { "b": {}, "a": {} } }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        let features = raw.features.expect("features should be Some");
        let keys: Vec<&str> = features.keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, vec!["b", "a"]);
    }

    proptest! {
        #[test]
        fn parse_config_file_never_panics(s in ".*") {
            let file = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(file.path(), &s).unwrap();
            let _ = parse_config_file(file.path(), false);
        }
    }
}
