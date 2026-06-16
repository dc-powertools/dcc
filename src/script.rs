use std::collections::HashMap;
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;

use crate::{
    cache::CacheDir,
    config::{self, vars::CONTAINER_WORKSPACE},
    docker,
    features::{self, FeatureRuntimeConfig},
    profile::{ContainerName, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn run_script(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    script_arg: &str,
    strict: bool,
) -> anyhow::Result<()> {
    let cache_dir = CacheDir::new(workspace, profile);
    let config = config::load_config(config_path, workspace, &cache_dir, strict)
        .with_context(|| format!("failed to load config `{}`", config_path.display()))?;

    let container = ContainerName::new(workspace, profile);
    let image_tag = container.as_image_tag();

    if !docker::inspect_running(container.as_str()).await? {
        anyhow::bail!(
            "container `{}` is not running; start it with `dcc exec`",
            container.as_str()
        );
    }

    let feature_runtime = match docker::inspect_image_label(image_tag.as_str()).await? {
        None => FeatureRuntimeConfig::default(),
        Some(ref json) => features::parse_runtime_from_label(json).with_context(|| {
            format!("failed to parse devcontainer.metadata label from image `{image_tag}`")
        })?,
    };

    let cmd = resolve_script(
        script_arg,
        &config.scripts,
        &feature_runtime.feature_scripts,
    )
    .with_context(|| format!("failed to resolve script `{script_arg}`"))?;

    let argv = vec!["/bin/sh".to_string(), "-c".to_string(), cmd.to_string()];
    let status = docker::exec(
        container.as_str(),
        &config.container_user,
        CONTAINER_WORKSPACE,
        &argv,
    )
    .await?;

    std::process::exit(status.code().unwrap_or(1));
}

/// Resolves a script argument to its shell command string.
///
/// The argument may be:
/// - `build` — unqualified: search devcontainer and all features; error if ambiguous
/// - `:build` — explicitly the devcontainer's script named `build`
/// - `node:build` — explicitly the script named `build` from the feature with short id `node`
pub(crate) fn resolve_script<'a>(
    arg: &str,
    dc_scripts: &'a HashMap<String, String>,
    feature_scripts: &'a [(String, IndexMap<String, String>)],
) -> anyhow::Result<&'a str> {
    match parse_script_arg(arg) {
        ParsedArg::DevcontainerQualified(name) => dc_scripts
            .get(name)
            .map(String::as_str)
            .with_context(|| format!("devcontainer has no script named `{name}`")),

        ParsedArg::FeatureQualified(prefix, name) => {
            let matches: Vec<&IndexMap<String, String>> = feature_scripts
                .iter()
                .filter(|(id, _)| id == prefix)
                .map(|(_, s)| s)
                .collect();
            match matches.len() {
                0 => anyhow::bail!("no feature with id `{prefix}`"),
                1 => matches[0]
                    .get(name)
                    .map(String::as_str)
                    .with_context(|| format!("feature `{prefix}` has no script named `{name}`")),
                _ => anyhow::bail!(
                    "multiple features share the id `{prefix}`; this is a configuration error"
                ),
            }
        }

        ParsedArg::Unqualified(name) => {
            let dc_cmd = dc_scripts.get(name).map(String::as_str);
            let feat_matches: Vec<(&str, &str)> = feature_scripts
                .iter()
                .filter_map(|(id, scripts)| {
                    scripts.get(name).map(|cmd| (id.as_str(), cmd.as_str()))
                })
                .collect();

            let total = dc_cmd.is_some() as usize + feat_matches.len();
            match total {
                0 => anyhow::bail!(
                    "no script named `{name}`; available: {}",
                    list_all_scripts(dc_scripts, feature_scripts)
                ),
                1 => {
                    if let Some(cmd) = dc_cmd {
                        Ok(cmd)
                    } else {
                        Ok(feat_matches[0].1)
                    }
                }
                _ => {
                    let mut qualified: Vec<String> = Vec::new();
                    if dc_cmd.is_some() {
                        qualified.push(format!(":{name}"));
                    }
                    for (id, _) in &feat_matches {
                        qualified.push(format!("{id}:{name}"));
                    }
                    anyhow::bail!(
                        "script `{name}` is defined in multiple sources; \
                         use a qualified name: {}",
                        qualified.join(", ")
                    )
                }
            }
        }
    }
}

enum ParsedArg<'a> {
    Unqualified(&'a str),
    DevcontainerQualified(&'a str),
    FeatureQualified(&'a str, &'a str),
}

fn parse_script_arg(arg: &str) -> ParsedArg<'_> {
    if let Some(rest) = arg.strip_prefix(':') {
        ParsedArg::DevcontainerQualified(rest)
    } else if let Some(pos) = arg.find(':') {
        ParsedArg::FeatureQualified(&arg[..pos], &arg[pos + 1..])
    } else {
        ParsedArg::Unqualified(arg)
    }
}

fn list_all_scripts(
    dc_scripts: &HashMap<String, String>,
    feature_scripts: &[(String, IndexMap<String, String>)],
) -> String {
    let mut all: Vec<String> = Vec::new();
    let mut dc_names: Vec<&str> = dc_scripts.keys().map(String::as_str).collect();
    dc_names.sort_unstable();
    for name in dc_names {
        all.push(format!(":{name}"));
    }
    for (id, scripts) in feature_scripts {
        let mut names: Vec<&str> = scripts.keys().map(String::as_str).collect();
        names.sort_unstable();
        for name in names {
            all.push(format!("{id}:{name}"));
        }
    }
    if all.is_empty() {
        "(none)".to_string()
    } else {
        all.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dc(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn feat(id: &str, pairs: &[(&str, &str)]) -> (String, IndexMap<String, String>) {
        let map = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        (id.to_string(), map)
    }

    // --- resolve_script ---

    #[test]
    fn unqualified_only_in_devcontainer() {
        let dc = dc(&[("build", "cargo build")]);
        let result = resolve_script("build", &dc, &[]).unwrap();
        assert_eq!(result, "cargo build");
    }

    #[test]
    fn unqualified_only_in_feature() {
        let dc = dc(&[]);
        let feats = [feat("node", &[("build", "npm run build")])];
        let result = resolve_script("build", &dc, &feats).unwrap();
        assert_eq!(result, "npm run build");
    }

    #[test]
    fn unqualified_collision_errors_with_qualified_suggestions() {
        let dc = dc(&[("build", "make all")]);
        let feats = [feat("node", &[("build", "npm run build")])];
        let err = resolve_script("build", &dc, &feats).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(":build"), "expected ':build' in: {msg}");
        assert!(
            msg.contains("node:build"),
            "expected 'node:build' in: {msg}"
        );
    }

    #[test]
    fn unqualified_not_found_lists_available() {
        let dc = dc(&[("test", "cargo test")]);
        let feats = [feat("node", &[("lint", "eslint .")])];
        let err = resolve_script("missing", &dc, &feats).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(":test"), "expected ':test' in: {msg}");
        assert!(msg.contains("node:lint"), "expected 'node:lint' in: {msg}");
    }

    #[test]
    fn devcontainer_qualified_found() {
        let dc = dc(&[("build", "make all")]);
        let feats = [feat("node", &[("build", "npm run build")])];
        let result = resolve_script(":build", &dc, &feats).unwrap();
        assert_eq!(result, "make all");
    }

    #[test]
    fn devcontainer_qualified_not_found() {
        let dc = dc(&[]);
        let err = resolve_script(":missing", &dc, &[]).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn feature_qualified_found() {
        let dc = dc(&[("build", "make all")]);
        let feats = [feat("node", &[("build", "npm run build")])];
        let result = resolve_script("node:build", &dc, &feats).unwrap();
        assert_eq!(result, "npm run build");
    }

    #[test]
    fn feature_qualified_unknown_feature() {
        let feats = [feat("node", &[("build", "npm run build")])];
        let err = resolve_script("rust:build", &dc(&[]), &feats).unwrap_err();
        assert!(err.to_string().contains("rust"));
    }

    #[test]
    fn feature_qualified_unknown_script_in_known_feature() {
        let feats = [feat("node", &[("build", "npm run build")])];
        let err = resolve_script("node:missing", &dc(&[]), &feats).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn two_features_no_collision_both_accessible() {
        let feats = [
            feat("rust", &[("build", "cargo build")]),
            feat("node", &[("build", "npm run build")]),
        ];
        // Unqualified: collision
        assert!(resolve_script("build", &dc(&[]), &feats).is_err());
        // Qualified: each resolves correctly
        assert_eq!(
            resolve_script("rust:build", &dc(&[]), &feats).unwrap(),
            "cargo build"
        );
        assert_eq!(
            resolve_script("node:build", &dc(&[]), &feats).unwrap(),
            "npm run build"
        );
    }

    // --- list_all_scripts ---

    #[test]
    fn list_empty_returns_none() {
        assert_eq!(list_all_scripts(&dc(&[]), &[]), "(none)");
    }

    #[test]
    fn list_dc_scripts_prefixed_with_colon() {
        let result = list_all_scripts(&dc(&[("build", "x"), ("test", "y")]), &[]);
        assert!(result.contains(":build"));
        assert!(result.contains(":test"));
    }

    #[test]
    fn list_feature_scripts_prefixed_with_id() {
        let feats = [feat("node", &[("lint", "eslint .")])];
        let result = list_all_scripts(&dc(&[]), &feats);
        assert!(result.contains("node:lint"));
    }
}
