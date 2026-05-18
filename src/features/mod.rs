use std::collections::HashSet;
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;

use crate::config::DevcontainerConfig;

use self::{context::FeatureContext, oci::OciClient};

pub(crate) mod context;
mod local;
pub(crate) mod oci;

/// Builds the env-var map for a feature by merging defaults from `devcontainer-feature.json`
/// with user-supplied options. Option keys are uppercased to form the variable name.
/// Shared by both OCI and local feature loading.
fn build_env(
    feature_json: Option<&[u8]>,
    user_options: &serde_json::Value,
) -> IndexMap<String, String> {
    let mut env = IndexMap::new();

    // Extract defaults from devcontainer-feature.json
    if let Some(bytes) = feature_json {
        if let Ok(meta) = serde_json::from_slice::<serde_json::Value>(bytes) {
            if let Some(options) = meta.get("options").and_then(|v| v.as_object()) {
                for (key, schema) in options {
                    let env_key = key.to_uppercase();
                    let default_val = schema
                        .get("default")
                        .map(json_value_to_string)
                        .unwrap_or_default();
                    env.insert(env_key, default_val);
                }
            }
        }
    }

    // Overlay user-supplied options (override defaults)
    if let Some(obj) = user_options.as_object() {
        for (key, val) in obj {
            env.insert(key.to_uppercase(), json_value_to_string(val));
        }
    }

    env
}

fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Returns true when `reference` is a local path rather than an OCI registry reference.
/// Local paths start with `./` or `../`, per the devcontainer spec.
fn is_local_feature(reference: &str) -> bool {
    reference.starts_with("./") || reference.starts_with("../")
}

/// Loads all features and assembles an in-memory Docker build context tar.
///
/// `config_dir` is the directory containing the devcontainer config file; local
/// feature paths (`./…` or `../…`) are resolved relative to it.
///
/// Returns the raw (uncompressed) tar bytes for piping to `docker build -`.
pub(crate) async fn build_context(
    config: &DevcontainerConfig,
    config_dir: &Path,
) -> anyhow::Result<Vec<u8>> {
    let mut client = OciClient::new().context("failed to initialize OCI HTTP client")?;
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut feature_contexts: Vec<FeatureContext> = Vec::new();

    for (reference, options) in &config.features {
        let downloaded = if is_local_feature(reference) {
            local::load_local_feature(reference, config_dir, options)
                .with_context(|| format!("failed to load local feature `{reference}`"))?
        } else {
            client
                .download_feature(reference, options)
                .await
                .with_context(|| format!("failed to download feature `{reference}`"))?
        };

        let id = context::unique_feature_id(reference, &mut seen_ids);

        feature_contexts.push(FeatureContext {
            id,
            install_sh: downloaded.install_sh,
            feature_json: downloaded.feature_json.unwrap_or_default(),
            env_vars: downloaded.env,
            extra_files: downloaded.extra_files,
        });
    }

    context::build_context(
        &config.image,
        &feature_contexts,
        config.container_user.as_deref(),
    )
    .context("failed to assemble Docker build context")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // --- unique_feature_id (via context module) ---

    #[test]
    fn feature_id_slug() {
        let mut seen = HashSet::new();
        let id = context::unique_feature_id("ghcr.io/devcontainers/features/node:1", &mut seen);
        assert_eq!(id, "ghcr-io-devcontainers-features-node-1");
    }

    #[test]
    fn feature_id_collision_handled() {
        let mut seen = HashSet::new();
        let id1 = context::unique_feature_id("ref-one", &mut seen);
        let id2 = context::unique_feature_id("ref.one", &mut seen);
        assert_eq!(id1, "ref-one");
        assert_ne!(id2, "ref-one");
        assert!(id2.starts_with("ref-one-"));
    }

    // --- build_env ---

    #[test]
    fn build_env_defaults_applied() {
        let feature_json = serde_json::json!({
            "options": { "version": { "type": "string", "default": "lts" } }
        });
        let bytes = serde_json::to_vec(&feature_json).unwrap();
        let env = build_env(Some(&bytes), &serde_json::json!({}));
        assert_eq!(env.get("VERSION"), Some(&"lts".to_string()));
    }

    #[test]
    fn build_env_user_overrides_default() {
        let feature_json = serde_json::json!({
            "options": { "version": { "default": "lts" } }
        });
        let bytes = serde_json::to_vec(&feature_json).unwrap();
        let env = build_env(Some(&bytes), &serde_json::json!({"version": "20"}));
        assert_eq!(env.get("VERSION"), Some(&"20".to_string()));
    }

    #[test]
    fn build_env_no_feature_json() {
        let env = build_env(None, &serde_json::json!({"version": "20"}));
        assert_eq!(env.get("VERSION"), Some(&"20".to_string()));
    }

    #[test]
    fn build_env_key_uppercased() {
        let env = build_env(None, &serde_json::json!({"nodeVersion": "20"}));
        assert!(env.contains_key("NODEVERSION"));
    }

    // --- is_local_feature ---

    #[test]
    fn local_feature_dot_slash() {
        assert!(is_local_feature("./my-feature"));
        assert!(is_local_feature("./nested/my-feature"));
    }

    #[test]
    fn local_feature_dot_dot_slash() {
        assert!(is_local_feature("../sibling-feature"));
        assert!(is_local_feature("../../deep/feature"));
    }

    #[test]
    fn oci_reference_is_not_local() {
        assert!(!is_local_feature("ghcr.io/devcontainers/features/node:1"));
        assert!(!is_local_feature("my-registry.io/owner/repo:latest"));
    }

    #[test]
    fn bare_name_is_not_local() {
        assert!(!is_local_feature("my-feature"));
        assert!(!is_local_feature("node"));
    }
}
