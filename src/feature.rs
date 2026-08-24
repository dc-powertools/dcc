use std::path::Path;

use anyhow::Context as _;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::{
    cli::OutputFormat, config, dry_run::DryRunReport, profile::ProfileName, workspace::Workspace,
};

pub(crate) struct FeatureOptions {
    pub(crate) add: Vec<String>,
    pub(crate) remove: Vec<String>,
    pub(crate) strict: bool,
    pub(crate) dry_run: bool,
    pub(crate) debug: bool,
    pub(crate) format: OutputFormat,
}

#[derive(Debug, Serialize)]
struct FeatureEditSummary {
    added: Vec<String>,
    already_present: Vec<String>,
    removed: Vec<String>,
    not_present: Vec<String>,
}

impl FeatureEditSummary {
    fn changed(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }
}

pub(crate) fn update_features(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    opts: FeatureOptions,
) -> anyhow::Result<()> {
    let add = normalize_feature_refs(opts.add, "--add")?;
    let remove = normalize_feature_refs(opts.remove, "--remove")?;
    if add.is_empty() && remove.is_empty() {
        anyhow::bail!("feature requires at least one --add or --remove value");
    }

    config::parse_config_file(config_path, opts.strict).with_context(|| {
        format!(
            "failed to validate profile config `{}`",
            config_path.display()
        )
    })?;

    let contents = std::fs::read_to_string(config_path)
        .with_context(|| format!("failed to read {}", config_path.display()))?;
    let (updated, summary) = edit_feature_json(&contents, &add, &remove)
        .with_context(|| format!("failed to update features in `{}`", config_path.display()))?;

    if opts.debug {
        eprintln!("dcc debug: command `feature`");
        eprintln!("dcc debug: profile `{}`", profile.as_str());
        eprintln!("dcc debug: config `{}`", config_path.display());
        eprintln!("dcc debug: add `{}`", add.join(", "));
        eprintln!("dcc debug: remove `{}`", remove.join(", "));
        eprintln!("dcc debug: changed `{}`", summary.changed());
    }

    if opts.dry_run {
        let command = feature_command_label(&add, &remove);
        DryRunReport::new(
            command,
            workspace,
            profile,
            config_path,
            vec!["profile config parsed", "feature edits planned"],
            Vec::<String>::new(),
        )
        .print(opts.format)?;
        return Ok(());
    }

    if summary.changed() {
        std::fs::write(config_path, updated)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }

    print_summary(&summary, opts.format)
}

fn feature_command_label(add: &[String], remove: &[String]) -> String {
    let mut parts = vec!["feature".to_string()];
    for feature in add {
        parts.push(format!("--add {feature}"));
    }
    for feature in remove {
        parts.push(format!("--remove {feature}"));
    }
    parts.join(" ")
}

fn normalize_feature_refs(values: Vec<String>, flag: &str) -> anyhow::Result<Vec<String>> {
    let mut normalized = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for value in values {
        if value.trim().is_empty() {
            anyhow::bail!("{flag} requires a non-empty feature reference");
        }
        if seen.insert(value.clone()) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

fn edit_feature_json(
    contents: &str,
    add: &[String],
    remove: &[String],
) -> anyhow::Result<(String, FeatureEditSummary)> {
    let mut root: Value = json5::from_str(contents).context("failed to parse JSONC")?;
    let object = root
        .as_object_mut()
        .context("devcontainer config must be a JSON object")?;

    let mut summary = FeatureEditSummary {
        added: Vec::new(),
        already_present: Vec::new(),
        removed: Vec::new(),
        not_present: Vec::new(),
    };

    for feature in remove {
        let removed = features_object_mut(object)
            .and_then(|features| features.remove(feature))
            .is_some();
        if removed {
            summary.removed.push(feature.clone());
        } else {
            summary.not_present.push(feature.clone());
        }
    }

    if !add.is_empty() {
        let features = ensure_features_object(object)?;
        for feature in add {
            if features.contains_key(feature) {
                summary.already_present.push(feature.clone());
            } else {
                features.insert(feature.clone(), Value::Object(Map::new()));
                summary.added.push(feature.clone());
            }
        }
    }

    let json = serde_json::to_string_pretty(&root).context("failed to serialize updated config")?;
    Ok((format!("{json}\n"), summary))
}

fn features_object_mut(object: &mut Map<String, Value>) -> Option<&mut Map<String, Value>> {
    object.get_mut("features").and_then(Value::as_object_mut)
}

fn ensure_features_object(
    object: &mut Map<String, Value>,
) -> anyhow::Result<&mut Map<String, Value>> {
    match object.get("features") {
        Some(Value::Object(_)) => {}
        Some(Value::Null) | None => {
            object.insert("features".to_string(), Value::Object(Map::new()));
        }
        Some(_) => anyhow::bail!("`features` must be a JSON object"),
    }

    object
        .get_mut("features")
        .and_then(Value::as_object_mut)
        .context("failed to create `features` object")
}

fn print_summary(summary: &FeatureEditSummary, format: OutputFormat) -> anyhow::Result<()> {
    match format {
        OutputFormat::Text => {
            for feature in &summary.added {
                println!("added feature {feature}");
            }
            for feature in &summary.already_present {
                println!("feature already present {feature}");
            }
            for feature in &summary.removed {
                println!("removed feature {feature}");
            }
            for feature in &summary.not_present {
                println!("feature not present {feature}");
            }
            if !summary.changed() {
                println!("profile features unchanged");
            }
            Ok(())
        }
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(summary)
                .context("failed to serialize feature edit summary")?;
            println!("{json}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_adds_features_to_existing_object() {
        let (updated, summary) = edit_feature_json(
            r#"{ "image": "rust:1", "features": { "a": { "version": "1" } } }"#,
            &["b".to_string()],
            &[],
        )
        .unwrap();
        let json: Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(json["features"]["a"]["version"], "1");
        assert_eq!(json["features"]["b"], serde_json::json!({}));
        assert_eq!(summary.added, vec!["b"]);
    }

    #[test]
    fn edit_removes_features_before_adding() {
        let (updated, summary) = edit_feature_json(
            r#"{ "image": "rust:1", "features": { "a": {}, "b": {} } }"#,
            &["c".to_string()],
            &["a".to_string()],
        )
        .unwrap();
        let json: Value = serde_json::from_str(&updated).unwrap();
        assert!(json["features"].get("a").is_none());
        assert_eq!(json["features"]["b"], serde_json::json!({}));
        assert_eq!(json["features"]["c"], serde_json::json!({}));
        assert_eq!(summary.removed, vec!["a"]);
        assert_eq!(summary.added, vec!["c"]);
    }

    #[test]
    fn edit_creates_features_object() {
        let (updated, summary) =
            edit_feature_json(r#"{ "image": "rust:1" }"#, &["a".to_string()], &[]).unwrap();
        let json: Value = serde_json::from_str(&updated).unwrap();
        assert_eq!(json["features"]["a"], serde_json::json!({}));
        assert_eq!(summary.added, vec!["a"]);
    }

    #[test]
    fn edit_rejects_non_object_features() {
        let err = edit_feature_json(
            r#"{ "image": "rust:1", "features": [] }"#,
            &["a".to_string()],
            &[],
        )
        .unwrap_err();
        assert!(err.to_string().contains("`features` must be a JSON object"));
    }
}
