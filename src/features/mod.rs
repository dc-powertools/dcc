use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;

use crate::{
    config::{vars::apply_container_env_substitution, DevcontainerConfig},
    lifecycle::{LifecycleHooks, HOOKS},
};

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
    /// Environment variables to bake into the image via Dockerfile `ENV` before
    /// this feature's install script runs.
    container_env: IndexMap<String, String>,
    /// Environment variables passed as runtime flags to `docker run`.
    /// Stored as raw templates; substitution is applied at `dcc run` time.
    remote_env: IndexMap<String, String>,
    /// Additional mounts to attach when the container starts.
    mounts: Vec<FeatureMount>,
    /// Soft ordering hint: install this feature after the listed feature IDs
    /// if those features are already in the installation set.
    installs_after: Vec<String>,
    /// Hard dependencies: features that must be installed before this one.
    /// Keys are feature references in the same format as `devcontainer.json`
    /// `features`; values are the options for each dependency.
    depends_on: IndexMap<String, serde_json::Value>,
    /// Lifecycle hooks contributed by this feature. Run before the
    /// devcontainer.json hook of the same type, in feature installation order.
    #[serde(flatten)]
    lifecycle: LifecycleHooks,
}

/// A mount from `devcontainer-feature.json`, in the JSON object form.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FeatureMount {
    #[serde(default, skip_serializing_if = "String::is_empty")]
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

/// Runtime contributions from installed features, parsed from the
/// `devcontainer.metadata` image label at `dcc run` time.
#[derive(Debug, Default)]
pub(crate) struct FeatureRuntimeConfig {
    /// Additional mounts to pass to `docker run`, in `--mount` template string form.
    /// Variable references (e.g. `${localCacheFolder}`) are substituted at run time.
    pub(crate) mounts: Vec<String>,
    /// Environment variables to pass as `-e KEY=VALUE` flags to `docker run`.
    /// Stored as raw templates; variable references are substituted at run time.
    pub(crate) remote_env: IndexMap<String, String>,
    /// Lifecycle hooks contributed by installed features, as
    /// `(feature reference, hooks)` pairs in installation order. Stored as
    /// raw templates; variable references are substituted at run time.
    pub(crate) feature_hooks: Vec<(String, LifecycleHooks)>,
}

/// One entry in the feature lockfile — the resolved state of a single feature.
#[derive(serde::Serialize)]
pub(crate) struct LockEntry {
    /// Feature reference exactly as written in `devcontainer.json`.
    #[serde(rename = "ref")]
    pub(crate) reference: String,
    /// Options supplied by the user (or by the declaring `dependsOn`).
    pub(crate) options: serde_json::Value,
    /// Content-addressed identifier of what was actually installed.
    /// OCI features: the layer blob digest (`sha256:…`) verified on download.
    /// Local features: `sha256:<hex>` of the `install.sh` content at build time.
    pub(crate) resolved: String,
    /// `true` when the feature was listed directly in `devcontainer.json`;
    /// `false` when it was pulled in transitively via `dependsOn`.
    pub(crate) direct: bool,
}

/// Return value of `build_context`.
pub(crate) struct FeatureBuildOutput {
    pub(crate) context_tar: Vec<u8>,
    /// Serialised `devcontainer.metadata` label JSON, or `None` when no feature
    /// contributed any runtime properties (mounts, command, remoteEnv).
    pub(crate) metadata_label: Option<String>,
    /// Lockfile entries in topological installation order.
    pub(crate) lock_entries: Vec<LockEntry>,
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
    locked_digests: &HashMap<String, String>,
) -> anyhow::Result<FeatureBuildOutput> {
    let mut client = OciClient::new().context("failed to initialize OCI HTTP client")?;

    // Phase 1: resolve the full feature set (dependsOn may add new features)
    let all = resolve_features(config, config_dir, &mut client, locked_digests).await?;

    // Phase 2: topological sort (dependsOn hard ordering + installsAfter hints)
    let order = topological_sort(&all)?;

    // Phase 3: build FeatureContexts and the devcontainer.metadata label entries
    let direct_refs: HashSet<&str> = config.features.keys().map(String::as_str).collect();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut feature_contexts: Vec<FeatureContext> = Vec::new();
    let mut label_entries: Vec<serde_json::Value> = Vec::new();
    let mut lock_entries: Vec<LockEntry> = Vec::new();

    for reference in &order {
        let entry = &all[reference];

        // Build the label entry for this feature (only fields with content are included)
        let mut label_entry = serde_json::json!({ "id": reference });

        if !entry.meta.mounts.is_empty() {
            label_entry["mounts"] = serde_json::to_value(&entry.meta.mounts)
                .context("failed to serialize feature mounts")?;
        }

        if !entry.meta.remote_env.is_empty() {
            label_entry["remoteEnv"] = serde_json::to_value(&entry.meta.remote_env)
                .context("failed to serialize feature remoteEnv")?;
        }

        for (name, get) in HOOKS {
            if let Some(cmd) = get(&entry.meta.lifecycle) {
                label_entry[name] = serde_json::to_value(cmd)
                    .context("failed to serialize feature lifecycle hook")?;
            }
        }

        // Only include the entry in the label if there are runtime contributions
        // beyond the always-present "id" field.
        if label_entry.as_object().is_some_and(|o| o.len() > 1) {
            label_entries.push(label_entry);
        }

        let id = context::unique_feature_id(reference, &mut seen_ids);

        // containerEnv: apply container-only substitution (no local vars)
        let container_env = entry
            .meta
            .container_env
            .iter()
            .map(|(k, v)| (k.clone(), apply_container_env_substitution(v)))
            .collect();

        feature_contexts.push(FeatureContext {
            id,
            install_sh: entry.downloaded.install_sh.clone(),
            feature_json: entry.downloaded.feature_json.clone().unwrap_or_default(),
            env_vars: entry.downloaded.env.clone(),
            container_env,
            extra_files: entry.downloaded.extra_files.clone(),
        });

        lock_entries.push(LockEntry {
            reference: reference.clone(),
            options: entry.user_options.clone(),
            resolved: entry.downloaded.resolved_digest.clone(),
            direct: direct_refs.contains(reference.as_str()),
        });
    }

    let mut devcontainer_env: Vec<(String, String)> = config
        .container_env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    devcontainer_env.sort_by(|a, b| a.0.cmp(&b.0));

    let context_tar = context::build_context(
        &config.image,
        &devcontainer_env,
        &feature_contexts,
        &config.container_user,
        !config.forward_ports.is_empty(),
    )
    .context("failed to assemble Docker build context")?;

    let metadata_label = if label_entries.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(&label_entries)
                .context("failed to serialize devcontainer.metadata label")?,
        )
    };

    Ok(FeatureBuildOutput {
        context_tar,
        metadata_label,
        lock_entries,
    })
}

/// Parses a `devcontainer.metadata` label value into a `FeatureRuntimeConfig`.
///
/// The label value must be a JSON array of feature contribution objects, or a
/// single object (normalised to a one-element array). Returns an error if the
/// JSON is malformed or a required field has an unexpected type.
pub(crate) fn parse_runtime_from_label(json: &str) -> anyhow::Result<FeatureRuntimeConfig> {
    let value: serde_json::Value = serde_json::from_str(json)
        .context("failed to parse devcontainer.metadata label as JSON")?;

    let entries = match value {
        serde_json::Value::Array(arr) => arr,
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => anyhow::bail!("devcontainer.metadata label must be a JSON array or object"),
    };

    let mut config = FeatureRuntimeConfig::default();

    for entry in &entries {
        // mounts: collect all; convert JSON objects to --mount template strings
        if let Some(mounts_val) = entry.get("mounts") {
            let mounts: Vec<FeatureMount> = serde_json::from_value(mounts_val.clone())
                .context("failed to parse 'mounts' in devcontainer.metadata label")?;
            for mount in mounts {
                config.mounts.push(mount.to_mount_string());
            }
        }

        // remoteEnv: last value per key wins
        if let Some(env_val) = entry.get("remoteEnv") {
            let env: IndexMap<String, String> = serde_json::from_value(env_val.clone())
                .context("failed to parse 'remoteEnv' in devcontainer.metadata label")?;
            config.remote_env.extend(env);
        }

        // lifecycle hooks: collect (id, hooks) for every entry declaring at least one
        let hooks: LifecycleHooks = serde_json::from_value(entry.clone())
            .context("failed to parse lifecycle hooks in devcontainer.metadata label")?;
        if !hooks.is_empty() {
            let id = entry
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            config.feature_hooks.push((id, hooks));
        }
    }

    Ok(config)
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
    locked_digests: &HashMap<String, String>,
) -> anyhow::Result<IndexMap<String, FeatureEntry>> {
    let mut all: IndexMap<String, FeatureEntry> = IndexMap::new();
    // Maps reference → options for every feature that has been enqueued.
    // Used to detect options conflicts even for features not yet processed.
    let mut queued: HashMap<String, serde_json::Value> = config
        .features
        .iter()
        .map(|(r, o)| (r.clone(), o.clone()))
        .collect();
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
            let locked = locked_digests.get(&reference).map(String::as_str);
            client
                .download_feature(&reference, &user_options, locked)
                .await
                .with_context(|| format!("failed to download feature `{reference}`"))?
        };

        let meta = parse_feature_meta(downloaded.feature_json.as_deref());

        for (dep_ref, dep_opts) in &meta.depends_on {
            if let Some(existing_opts) = queued.get(dep_ref) {
                // Already enqueued or processed — warn if options differ.
                let canonical_opts = all
                    .get(dep_ref)
                    .map(|e| &e.user_options)
                    .unwrap_or(existing_opts);
                if canonical_opts != dep_opts {
                    tracing::warn!(
                        dependency = dep_ref,
                        "feature dependency already present with different options; ignoring dependency's options"
                    );
                }
            } else {
                queued.insert(dep_ref.clone(), dep_opts.clone());
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
            .min_by_key(|(_, r)| {
                all.get_index_of(r.as_str())
                    .expect("ready queue only contains references from `all`")
            })
            .map(|(i, _)| i)
            .unwrap();
        let current = ready.swap_remove(pos);
        order.push(current.clone());

        for successor in successors.get(&current).into_iter().flatten() {
            let deg = in_degree
                .get_mut(successor)
                .expect("successors derived from edges built over `all`; all are in `in_degree`");
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
        assert!(meta.container_env.is_empty());
        assert!(meta.mounts.is_empty());
        assert!(meta.installs_after.is_empty());
        assert!(meta.depends_on.is_empty());
        assert!(meta.lifecycle.is_empty());
    }

    #[test]
    fn parse_meta_all_fields() {
        let json = serde_json::json!({
            "id": "my-feature",
            "containerEnv": { "MY_VAR": "hello" },
            "mounts": [{ "source": "vol", "target": "/data", "type": "volume" }],
            "installsAfter": ["common-utils"],
            "dependsOn": { "ghcr.io/owner/repo/dep:1": {} },
            "onCreateCommand": "echo on-create",
            "updateContentCommand": ["echo", "update-content"],
            "postCreateCommand": "echo post-create",
            "postStartCommand": "echo post-start",
            "postAttachCommand": { "a": "echo a", "b": ["echo", "b"] }
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let meta = parse_feature_meta(Some(&bytes));
        assert_eq!(meta.id.as_deref(), Some("my-feature"));
        assert_eq!(
            meta.container_env.get("MY_VAR").map(String::as_str),
            Some("hello")
        );
        assert_eq!(meta.mounts.len(), 1);
        assert_eq!(meta.installs_after, vec!["common-utils"]);
        assert!(meta.depends_on.contains_key("ghcr.io/owner/repo/dep:1"));
        assert_eq!(
            meta.lifecycle.on_create_command,
            Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo on-create".to_string()
            ))
        );
        assert_eq!(
            meta.lifecycle.update_content_command,
            Some(crate::lifecycle::LifecycleCommand::Exec(vec![
                "echo".to_string(),
                "update-content".to_string()
            ]))
        );
        assert_eq!(
            meta.lifecycle.post_create_command,
            Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo post-create".to_string()
            ))
        );
        assert_eq!(
            meta.lifecycle.post_start_command,
            Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo post-start".to_string()
            ))
        );
        assert!(matches!(
            meta.lifecycle.post_attach_command,
            Some(crate::lifecycle::LifecycleCommand::Parallel(_))
        ));
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
                resolved_digest: String::new(),
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
        // a wants to come after a feature with id "common"
        let a_meta = FeatureMeta {
            installs_after: vec!["common".into()],
            ..Default::default()
        };
        all.insert("a:1".into(), entry(a_meta));
        let b_meta = FeatureMeta {
            id: Some("common".into()),
            ..Default::default()
        };
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

    // Helper: extract the Dockerfile from a tar build context.
    fn extract_dockerfile(tar_bytes: &[u8]) -> String {
        let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_str().unwrap().to_owned();
            if path == "Dockerfile" {
                let mut contents = String::new();
                std::io::Read::read_to_string(&mut entry, &mut contents).unwrap();
                return contents;
            }
        }
        panic!("Dockerfile not found in tar");
    }

    fn local_config(
        tmp: &std::path::Path,
        feature_json: &[u8],
    ) -> crate::config::DevcontainerConfig {
        use crate::config::DevcontainerConfig;
        use std::collections::HashMap;
        let feature_dir = tmp.join("local-feat");
        std::fs::create_dir_all(&feature_dir).unwrap();
        std::fs::write(feature_dir.join("install.sh"), b"#!/bin/sh\n").unwrap();
        std::fs::write(feature_dir.join("devcontainer-feature.json"), feature_json).unwrap();
        let mut features = IndexMap::new();
        features.insert("./local-feat".to_string(), serde_json::json!({}));
        DevcontainerConfig {
            image: "rust:1".to_string(),
            features,
            container_env: HashMap::new(),
            remote_env: HashMap::new(),
            container_user: "dev".to_string(),
            mounts: vec![],
            forward_ports: vec![],
            initialize_command: None,
            lifecycle: LifecycleHooks::default(),
        }
    }

    #[tokio::test]
    async fn container_env_variables_substituted_in_build_context() {
        let tmp = tempfile::tempdir().unwrap();
        let config = local_config(
            tmp.path(),
            br#"{"containerEnv":{"PROJECT_ROOT":"${localWorkspaceFolder}/src","CACHE_DIR":"${containerCacheFolder}/data"}}"#,
        );
        let output = build_context(&config, tmp.path(), &HashMap::new())
            .await
            .unwrap();
        let dockerfile = extract_dockerfile(&output.context_tar);
        // ${localWorkspaceFolder} is unknown in container-only context — left as-is
        assert!(
            dockerfile.contains("PROJECT_ROOT='${localWorkspaceFolder}/src'"),
            "expected literal PROJECT_ROOT in dockerfile, got:\n{dockerfile}"
        );
        // ${containerCacheFolder} is always substituted
        assert!(
            dockerfile.contains("CACHE_DIR='/cache/data'"),
            "expected substituted CACHE_DIR in dockerfile, got:\n{dockerfile}"
        );
    }

    #[tokio::test]
    async fn feature_install_sees_devcontainer_and_feature_container_env() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = local_config(tmp.path(), br#"{"containerEnv":{"FEAT_VAR":"feat_value"}}"#);
        config
            .container_env
            .insert("DC_VAR".to_string(), "dc_value".to_string());
        let output = build_context(&config, tmp.path(), &HashMap::new())
            .await
            .unwrap();
        let dockerfile = extract_dockerfile(&output.context_tar);

        let dc_pos = dockerfile
            .find("ENV DC_VAR='dc_value'")
            .expect("devcontainer.json containerEnv should be set");
        let feat_pos = dockerfile
            .find("ENV FEAT_VAR='feat_value'")
            .expect("devcontainer-feature.json containerEnv should be set");
        let install_pos = dockerfile
            .find("RUN chmod +x")
            .expect("feature install RUN step should be present");

        assert!(
            dc_pos < install_pos && feat_pos < install_pos,
            "both containerEnv sources must be set via ENV before the feature install runs, got:\n{dockerfile}"
        );
    }

    #[tokio::test]
    async fn feature_remote_env_stored_as_raw_templates() {
        let tmp = tempfile::tempdir().unwrap();
        let config = local_config(
            tmp.path(),
            br#"{"remoteEnv":{"TOKEN":"${localCacheFolder}/tok"}}"#,
        );
        let output = build_context(&config, tmp.path(), &HashMap::new())
            .await
            .unwrap();
        let label = output.metadata_label.expect("expected metadata label");
        let runtime = parse_runtime_from_label(&label).unwrap();
        // remoteEnv is stored as a raw template — variable not substituted
        assert_eq!(
            runtime.remote_env.get("TOKEN"),
            Some(&"${localCacheFolder}/tok".to_string())
        );
    }

    #[tokio::test]
    async fn feature_hook_only_feature_included_in_label() {
        let tmp = tempfile::tempdir().unwrap();
        let config = local_config(
            tmp.path(),
            br#"{"postCreateCommand":"echo hello from feature"}"#,
        );
        let output = build_context(&config, tmp.path(), &HashMap::new())
            .await
            .unwrap();
        let label = output
            .metadata_label
            .expect("hook-only feature should still produce a metadata label");
        let runtime = parse_runtime_from_label(&label).unwrap();
        assert_eq!(runtime.feature_hooks.len(), 1);
        let (id, hooks) = &runtime.feature_hooks[0];
        assert_eq!(id, "./local-feat");
        assert_eq!(
            hooks.post_create_command,
            Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo hello from feature".to_string()
            ))
        );
    }

    // --- parse_runtime_from_label ---

    #[test]
    fn parse_label_mounts_converted_to_template_strings() {
        let json = r#"[{"id":"feat","mounts":[{"type":"bind","source":"${localCacheFolder}/x","target":"/x"}]}]"#;
        let config = parse_runtime_from_label(json).unwrap();
        assert_eq!(
            config.mounts,
            vec!["type=bind,source=${localCacheFolder}/x,target=/x"]
        );
    }

    #[test]
    fn parse_label_remote_env_last_value_per_key_wins() {
        let json =
            r#"[{"id":"a","remoteEnv":{"KEY":"first"}},{"id":"b","remoteEnv":{"KEY":"second"}}]"#;
        let config = parse_runtime_from_label(json).unwrap();
        assert_eq!(
            config.remote_env.get("KEY").map(String::as_str),
            Some("second")
        );
    }

    #[test]
    fn parse_label_empty_array_gives_default() {
        let config = parse_runtime_from_label("[]").unwrap();
        assert!(config.mounts.is_empty());
        assert!(config.remote_env.is_empty());
    }

    #[test]
    fn parse_label_bare_object_normalised_to_array() {
        let json = r#"{"id":"feat","mounts":[{"type":"volume","source":"vol","target":"/data"}]}"#;
        let config = parse_runtime_from_label(json).unwrap();
        assert_eq!(config.mounts.len(), 1);
    }

    #[test]
    fn parse_label_invalid_json_errors() {
        assert!(parse_runtime_from_label("{not valid json").is_err());
    }

    #[test]
    fn parse_label_wrong_root_type_errors() {
        assert!(parse_runtime_from_label("\"just a string\"").is_err());
    }

    #[test]
    fn parse_label_volume_mount_omits_source() {
        let json = r#"[{"id":"feat","mounts":[{"type":"volume","target":"/data"}]}]"#;
        let config = parse_runtime_from_label(json).unwrap();
        // source is empty so it is omitted from the --mount string
        assert_eq!(config.mounts, vec!["type=volume,source=,target=/data"]);
    }

    #[test]
    fn parse_label_feature_hooks_round_trip_preserves_order() {
        let json = r#"[
            {"id":"feat-a","postCreateCommand":"echo a"},
            {"id":"feat-b","postStartCommand":["echo","b"]}
        ]"#;
        let config = parse_runtime_from_label(json).unwrap();
        assert_eq!(config.feature_hooks.len(), 2);
        assert_eq!(config.feature_hooks[0].0, "feat-a");
        assert_eq!(
            config.feature_hooks[0].1.post_create_command,
            Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo a".to_string()
            ))
        );
        assert_eq!(config.feature_hooks[1].0, "feat-b");
        assert_eq!(
            config.feature_hooks[1].1.post_start_command,
            Some(crate::lifecycle::LifecycleCommand::Exec(vec![
                "echo".to_string(),
                "b".to_string()
            ]))
        );
    }

    #[test]
    fn parse_label_entries_without_hooks_excluded_from_feature_hooks() {
        let json = r#"[{"id":"feat","command":["/ep.sh"]}]"#;
        let config = parse_runtime_from_label(json).unwrap();
        assert!(config.feature_hooks.is_empty());
    }
}
