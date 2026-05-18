use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;

use crate::config::DevcontainerConfig;

use self::{context::FeatureContext, oci::OciClient};

pub(crate) mod context;
mod local;
pub(crate) mod oci;

// ── Feature metadata ──────────────────────────────────────────────────────────

/// Properties read from a feature's `devcontainer-feature.json` beyond the
/// `options` that drive `install.sh` env vars.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct FeatureMeta {
    /// The feature's canonical identifier within its repository (e.g. `"node"`).
    /// Used to resolve `installsAfter` references.
    id: Option<String>,
    /// Script(s) to run as the container entrypoint after the feature installs.
    entrypoint: Option<Vec<String>>,
    /// Environment variables to bake into the image via Dockerfile `ENV` before
    /// this feature's install script runs.
    container_env: IndexMap<String, String>,
    /// Additional mounts to attach when the container starts.
    mounts: Vec<FeatureMount>,
    /// Soft ordering hint: install this feature after the listed feature IDs
    /// if those features are already in the installation set.
    installs_after: Vec<String>,
    /// Hard dependencies: features that must be installed before this one.
    /// Keys are feature references in the same format as `devcontainer.json`
    /// `features`; values are the options for each dependency.
    depends_on: IndexMap<String, serde_json::Value>,
}

/// A mount from `devcontainer-feature.json`, in the JSON object form.
#[derive(Debug, serde::Deserialize)]
struct FeatureMount {
    #[serde(default)]
    source: String,
    target: String,
    #[serde(rename = "type")]
    mount_type: String,
}

impl FeatureMount {
    /// Converts to the `--mount` string form accepted by `docker run`.
    fn to_mount_string(&self) -> String {
        format!(
            "type={},source={},target={}",
            self.mount_type, self.source, self.target
        )
    }
}

// ── Public return types ───────────────────────────────────────────────────────

/// Runtime contributions from installed features, written to the cache dir at
/// `dcc build` time and read at `dcc run` time.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct FeatureRuntimeConfig {
    /// Additional mounts to pass to `docker run`, in `--mount` string form.
    #[serde(default)]
    pub(crate) mounts: Vec<String>,
    /// Entrypoint contributed by features. When multiple features declare an
    /// entrypoint, the last one in installation order wins (with a warning).
    /// `None` when no installed feature declares an entrypoint.
    pub(crate) entrypoint: Option<Vec<String>>,
}

/// Return value of `build_context`.
pub(crate) struct FeatureBuildOutput {
    pub(crate) context_tar: Vec<u8>,
    pub(crate) runtime: FeatureRuntimeConfig,
}

// ── Internal types ────────────────────────────────────────────────────────────

struct FeatureEntry {
    user_options: serde_json::Value,
    downloaded: oci::DownloadedFeature,
    meta: FeatureMeta,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Resolves feature dependencies, determines installation order, and assembles
/// an in-memory Docker build context tar.
///
/// `config_dir` is the directory containing the devcontainer config file; local
/// feature paths (`./…` or `../…`) are resolved relative to it.
pub(crate) async fn build_context(
    config: &DevcontainerConfig,
    config_dir: &Path,
) -> anyhow::Result<FeatureBuildOutput> {
    let mut client = OciClient::new().context("failed to initialize OCI HTTP client")?;

    // Phase 1: resolve the full feature set (dependsOn may add new features)
    let all = resolve_features(config, config_dir, &mut client).await?;

    // Phase 2: topological sort (dependsOn hard ordering + installsAfter hints)
    let order = topological_sort(&all)?;

    // Phase 3: build FeatureContexts and collect runtime contributions
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut feature_contexts: Vec<FeatureContext> = Vec::new();
    let mut runtime = FeatureRuntimeConfig::default();

    for reference in &order {
        let entry = &all[reference];

        // Entrypoint: last wins, warn whenever a value is replaced
        if let Some(ep) = &entry.meta.entrypoint {
            if let Some(prev) = &runtime.entrypoint {
                tracing::warn!(
                    feature = reference,
                    clobbered = ?prev,
                    replacement = ?ep,
                    "feature entrypoint clobbered by later feature in installation order"
                );
            }
            runtime.entrypoint = Some(ep.clone());
        }

        // Mounts: collect all and convert to string form
        for mount in &entry.meta.mounts {
            runtime.mounts.push(mount.to_mount_string());
        }

        let id = context::unique_feature_id(reference, &mut seen_ids);

        feature_contexts.push(FeatureContext {
            id,
            install_sh: entry.downloaded.install_sh.clone(),
            feature_json: entry.downloaded.feature_json.clone().unwrap_or_default(),
            env_vars: entry.downloaded.env.clone(),
            container_env: entry.meta.container_env.clone(),
            extra_files: entry.downloaded.extra_files.clone(),
        });
    }

    let context_tar = context::build_context(
        &config.image,
        &feature_contexts,
        config.container_user.as_deref(),
    )
    .context("failed to assemble Docker build context")?;

    Ok(FeatureBuildOutput {
        context_tar,
        runtime,
    })
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn parse_feature_meta(feature_json: Option<&[u8]>) -> FeatureMeta {
    feature_json
        .and_then(|b| serde_json::from_slice(b).ok())
        .unwrap_or_default()
}

/// Resolves the full feature set by following `dependsOn` chains.
/// User-specified features are enqueued first, in declaration order.
/// Missing dependencies discovered via `dependsOn` are appended to the queue.
async fn resolve_features(
    config: &DevcontainerConfig,
    config_dir: &Path,
    client: &mut OciClient,
) -> anyhow::Result<IndexMap<String, FeatureEntry>> {
    let mut all: IndexMap<String, FeatureEntry> = IndexMap::new();
    let mut queued: HashSet<String> = config.features.keys().cloned().collect();
    let mut queue: VecDeque<(String, serde_json::Value)> = config
        .features
        .iter()
        .map(|(r, o)| (r.clone(), o.clone()))
        .collect();

    while let Some((reference, user_options)) = queue.pop_front() {
        let downloaded = if is_local_feature(&reference) {
            local::load_local_feature(&reference, config_dir, &user_options)
                .with_context(|| format!("failed to load local feature `{reference}`"))?
        } else {
            client
                .download_feature(&reference, &user_options)
                .await
                .with_context(|| format!("failed to download feature `{reference}`"))?
        };

        let meta = parse_feature_meta(downloaded.feature_json.as_deref());

        for (dep_ref, dep_opts) in &meta.depends_on {
            if let Some(existing) = all.get(dep_ref) {
                if &existing.user_options != dep_opts {
                    tracing::warn!(
                        dependency = dep_ref,
                        "feature dependency already present with different options; ignoring dependency's options"
                    );
                }
            } else if !queued.contains(dep_ref) {
                queued.insert(dep_ref.clone());
                queue.push_back((dep_ref.clone(), dep_opts.clone()));
            }
        }

        all.insert(
            reference,
            FeatureEntry {
                user_options,
                downloaded,
                meta,
            },
        );
    }

    Ok(all)
}

/// Topological sort of the resolved feature set using Kahn's algorithm.
///
/// Edges come from `dependsOn` (hard constraint) and `installsAfter` (soft
/// hint, applied only for features already present in the set). The user's
/// original declaration order is the tiebreaker for independent features.
///
/// Returns an error if a circular dependency is detected.
fn topological_sort(all: &IndexMap<String, FeatureEntry>) -> anyhow::Result<Vec<String>> {
    // Map feature id → reference(s) for installsAfter resolution
    let mut id_to_refs: HashMap<String, Vec<String>> = HashMap::new();
    for (reference, entry) in all {
        if let Some(id) = &entry.meta.id {
            id_to_refs
                .entry(id.clone())
                .or_default()
                .push(reference.clone());
        }
    }

    // Collect directed edges: (before, after) means `before` installs first
    let mut edges: HashSet<(String, String)> = HashSet::new();
    for (reference, entry) in all {
        for dep_ref in entry.meta.depends_on.keys() {
            if all.contains_key(dep_ref) && dep_ref != reference {
                edges.insert((dep_ref.clone(), reference.clone()));
            }
        }
        for id in &entry.meta.installs_after {
            for after_ref in id_to_refs.get(id).into_iter().flatten() {
                if after_ref != reference && all.contains_key(after_ref) {
                    edges.insert((after_ref.clone(), reference.clone()));
                }
            }
        }
    }

    // Build in-degree map and successor lists (owned Strings throughout)
    let mut in_degree: HashMap<String, usize> = all.keys().map(|r| (r.clone(), 0usize)).collect();
    let mut successors: HashMap<String, Vec<String>> =
        all.keys().map(|r| (r.clone(), Vec::new())).collect();
    for (before, after) in &edges {
        successors
            .entry(before.clone())
            .or_default()
            .push(after.clone());
        *in_degree.entry(after.clone()).or_default() += 1;
    }

    // Seed the ready queue with in-degree-0 nodes, in IndexMap insertion order
    let mut ready: Vec<String> = all.keys().filter(|r| in_degree[*r] == 0).cloned().collect();

    let mut order: Vec<String> = Vec::with_capacity(all.len());

    while !ready.is_empty() {
        // Pick the ready node that appears earliest in the original order
        let pos = ready
            .iter()
            .enumerate()
            .min_by_key(|(_, r)| all.get_index_of(r.as_str()).unwrap_or(usize::MAX))
            .map(|(i, _)| i)
            .unwrap();
        let current = ready.swap_remove(pos);
        order.push(current.clone());

        for successor in successors.get(&current).into_iter().flatten() {
            let deg = in_degree.get_mut(successor).unwrap();
            *deg -= 1;
            if *deg == 0 {
                ready.push(successor.clone());
            }
        }
    }

    if order.len() != all.len() {
        anyhow::bail!(
            "circular dependency detected among features; \
             check `dependsOn` and `installsAfter` declarations"
        );
    }

    Ok(order)
}

/// Builds the env-var map for a feature from `devcontainer-feature.json` option
/// defaults merged with user-supplied options. Option keys are uppercased.
fn build_env(
    feature_json: Option<&[u8]>,
    user_options: &serde_json::Value,
) -> IndexMap<String, String> {
    let mut env = IndexMap::new();

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

    // --- parse_feature_meta ---

    #[test]
    fn parse_meta_empty_bytes_gives_default() {
        let meta = parse_feature_meta(None);
        assert!(meta.id.is_none());
        assert!(meta.entrypoint.is_none());
        assert!(meta.container_env.is_empty());
        assert!(meta.mounts.is_empty());
        assert!(meta.installs_after.is_empty());
        assert!(meta.depends_on.is_empty());
    }

    #[test]
    fn parse_meta_all_fields() {
        let json = serde_json::json!({
            "id": "my-feature",
            "entrypoint": ["/usr/local/share/my-feature/entrypoint.sh"],
            "containerEnv": { "MY_VAR": "hello" },
            "mounts": [{ "source": "vol", "target": "/data", "type": "volume" }],
            "installsAfter": ["common-utils"],
            "dependsOn": { "ghcr.io/owner/repo/dep:1": {} }
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let meta = parse_feature_meta(Some(&bytes));
        assert_eq!(meta.id.as_deref(), Some("my-feature"));
        assert_eq!(
            meta.entrypoint.as_deref(),
            Some(&["/usr/local/share/my-feature/entrypoint.sh".to_string()][..])
        );
        assert_eq!(
            meta.container_env.get("MY_VAR").map(String::as_str),
            Some("hello")
        );
        assert_eq!(meta.mounts.len(), 1);
        assert_eq!(meta.installs_after, vec!["common-utils"]);
        assert!(meta.depends_on.contains_key("ghcr.io/owner/repo/dep:1"));
    }

    #[test]
    fn parse_meta_invalid_json_gives_default() {
        let meta = parse_feature_meta(Some(b"not valid json{{{"));
        assert!(meta.id.is_none());
    }

    // --- FeatureMount::to_mount_string ---

    #[test]
    fn mount_to_string_volume() {
        let m = FeatureMount {
            source: "my-volume".to_string(),
            target: "/data".to_string(),
            mount_type: "volume".to_string(),
        };
        assert_eq!(
            m.to_mount_string(),
            "type=volume,source=my-volume,target=/data"
        );
    }

    #[test]
    fn mount_to_string_bind() {
        let m = FeatureMount {
            source: "/host/path".to_string(),
            target: "/container/path".to_string(),
            mount_type: "bind".to_string(),
        };
        assert_eq!(
            m.to_mount_string(),
            "type=bind,source=/host/path,target=/container/path"
        );
    }

    // --- topological_sort ---

    fn entry(meta: FeatureMeta) -> FeatureEntry {
        FeatureEntry {
            user_options: serde_json::json!({}),
            downloaded: oci::DownloadedFeature {
                install_sh: vec![],
                feature_json: None,
                env: IndexMap::new(),
                extra_files: vec![],
            },
            meta,
        }
    }

    #[test]
    fn topo_sort_no_edges_preserves_insertion_order() {
        let mut all: IndexMap<String, FeatureEntry> = IndexMap::new();
        all.insert("a:1".into(), entry(FeatureMeta::default()));
        all.insert("b:1".into(), entry(FeatureMeta::default()));
        all.insert("c:1".into(), entry(FeatureMeta::default()));
        let order = topological_sort(&all).unwrap();
        assert_eq!(order, vec!["a:1", "b:1", "c:1"]);
    }

    #[test]
    fn topo_sort_depends_on_respected() {
        let mut all: IndexMap<String, FeatureEntry> = IndexMap::new();
        // b must come before a
        let mut a_meta = FeatureMeta::default();
        a_meta
            .depends_on
            .insert("b:1".into(), serde_json::json!({}));
        all.insert("a:1".into(), entry(a_meta));
        all.insert("b:1".into(), entry(FeatureMeta::default()));

        let order = topological_sort(&all).unwrap();
        let a_pos = order.iter().position(|r| r == "a:1").unwrap();
        let b_pos = order.iter().position(|r| r == "b:1").unwrap();
        assert!(b_pos < a_pos, "b must come before a");
    }

    #[test]
    fn topo_sort_installs_after_by_id() {
        let mut all: IndexMap<String, FeatureEntry> = IndexMap::new();
        let mut a_meta = FeatureMeta::default();
        // a wants to come after a feature with id "common"
        a_meta.installs_after = vec!["common".into()];
        all.insert("a:1".into(), entry(a_meta));
        let mut b_meta = FeatureMeta::default();
        b_meta.id = Some("common".into());
        all.insert("b:1".into(), entry(b_meta));

        let order = topological_sort(&all).unwrap();
        let a_pos = order.iter().position(|r| r == "a:1").unwrap();
        let b_pos = order.iter().position(|r| r == "b:1").unwrap();
        assert!(b_pos < a_pos, "b (id=common) must come before a");
    }

    #[test]
    fn topo_sort_cycle_errors() {
        let mut all: IndexMap<String, FeatureEntry> = IndexMap::new();
        let mut a_meta = FeatureMeta::default();
        a_meta
            .depends_on
            .insert("b:1".into(), serde_json::json!({}));
        let mut b_meta = FeatureMeta::default();
        b_meta
            .depends_on
            .insert("a:1".into(), serde_json::json!({}));
        all.insert("a:1".into(), entry(a_meta));
        all.insert("b:1".into(), entry(b_meta));

        assert!(topological_sort(&all).is_err());
    }
}
