use std::collections::HashMap;
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;
use serde::Deserialize;

use crate::{cache::CacheDir, workspace::Workspace};

pub(crate) mod merge;
pub(crate) mod resolve;
pub(crate) mod vars;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawConfig {
    pub(crate) extends: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) features: Option<IndexMap<String, serde_json::Value>>,
    pub(crate) container_env: Option<HashMap<String, String>>,
    pub(crate) container_user: Option<String>,
    pub(crate) mounts: Option<Vec<String>>,
    pub(crate) forward_ports: Option<Vec<u16>>,
    pub(crate) entrypoint: Option<Vec<String>>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub(crate) struct DevcontainerConfig {
    pub(crate) image: String,
    pub(crate) features: IndexMap<String, serde_json::Value>,
    pub(crate) container_env: HashMap<String, String>,
    pub(crate) container_user: String,
    pub(crate) mounts: Vec<String>,
    pub(crate) forward_ports: Vec<u16>,
    pub(crate) entrypoint: Option<Vec<String>>,
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
                "image": "rust:latest",
                "features": { "ghcr.io/devcontainers/features/node:1": { "version": "20" } },
                "containerEnv": { "FOO": "bar" },
                "containerUser": "dev",
                "mounts": ["type=bind,src=/tmp,dst=/tmp"],
                "forwardPorts": [8080, 3000],
                "entrypoint": ["/bin/bash", "-c", "echo hello"]
            }"#,
        );
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.extends.as_deref(), Some("base.json"));
        assert_eq!(raw.image.as_deref(), Some("rust:latest"));
        assert!(raw.features.is_some());
        assert!(raw.container_env.is_some());
        assert_eq!(raw.container_user.as_deref(), Some("dev"));
        assert_eq!(
            raw.mounts.as_deref(),
            Some(&[String::from("type=bind,src=/tmp,dst=/tmp")][..])
        );
        assert_eq!(raw.forward_ports.as_deref(), Some(&[8080u16, 3000u16][..]));
        assert_eq!(
            raw.entrypoint.as_deref(),
            Some(
                &[
                    "/bin/bash".to_string(),
                    "-c".to_string(),
                    "echo hello".to_string()
                ][..]
            )
        );
        assert!(raw.extra.is_empty());
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
        let file = write_temp(r#"{ "onCreateCommand": "foo" }"#);
        let result = parse_config_file(file.path(), false);
        assert!(result.is_ok());
        let raw = result.unwrap();
        assert!(raw.extra.contains_key("onCreateCommand"));
    }

    #[test]
    fn unknown_field_strict_mode() {
        let file = write_temp(r#"{ "onCreateCommand": "foo" }"#);
        let result = parse_config_file(file.path(), true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("onCreateCommand"));
    }

    #[test]
    fn empty_object() {
        let file = write_temp("{}");
        let raw = parse_config_file(file.path(), false).unwrap();
        assert!(raw.extends.is_none());
        assert!(raw.image.is_none());
        assert!(raw.features.is_none());
        assert!(raw.container_env.is_none());
        assert!(raw.container_user.is_none());
        assert!(raw.mounts.is_none());
        assert!(raw.forward_ports.is_none());
        assert!(raw.entrypoint.is_none());
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
