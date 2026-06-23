use std::collections::HashMap;
use std::path::Path;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config, docker,
    features::LockEntry,
    profile::{ContainerId, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn build(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    no_cache: bool,
    update: bool,
    strict: bool,
) -> anyhow::Result<()> {
    let cache_dir = CacheDir::new(workspace, profile);

    let config = config::load_config(config_path, workspace, &cache_dir, strict)
        .with_context(|| format!("failed to load config `{}`", config_path.display()))?;

    let container_id = ContainerId::new(workspace, profile);
    let image_tag = container_id.as_image_tag();

    if config.features.is_empty()
        && config.container_user == "root"
        && config.container_env.is_empty()
        && config.forward_ports.is_empty()
    {
        // Fast path: pull and retag without a Dockerfile build.
        // --no-cache is a no-op here: docker pull always contacts the registry.
        let _ = no_cache; // accepted for API uniformity; docker pull ignores it
        docker::pull(&config.image)
            .await
            .with_context(|| format!("failed to pull image `{}`", config.image))?;
        docker::tag(&config.image, image_tag.as_str())
            .await
            .with_context(|| {
                format!(
                    "failed to tag `{}` as `{}`",
                    config.image,
                    image_tag.as_str()
                )
            })?;
    } else {
        // Build path: generate Dockerfile, install features, create container user.
        let config_dir = config_path.parent().with_context(|| {
            format!(
                "config path `{}` has no parent directory",
                config_path.display()
            )
        })?;
        let locked_digests = if update {
            HashMap::new()
        } else {
            load_locked_digests(config_path)
        };
        let output = crate::features::build_context(&config, config_dir, &locked_digests)
            .await
            .context("failed to build feature context")?;
        docker::build(
            image_tag.as_str(),
            no_cache,
            output.context_tar,
            output.metadata_label.as_deref(),
        )
        .await
        .with_context(|| format!("failed to build image `{}`", image_tag.as_str()))?;

        write_lockfile(config_path, &output.lock_entries)?;
    }

    Ok(())
}

fn load_locked_digests(config_path: &Path) -> HashMap<String, String> {
    let lock_path = config_path.with_extension("lock");
    let Ok(content) = std::fs::read(&lock_path) else {
        return HashMap::new();
    };
    #[derive(serde::Deserialize)]
    struct Lock {
        features: Vec<Entry>,
    }
    #[derive(serde::Deserialize)]
    struct Entry {
        #[serde(rename = "ref")]
        reference: String,
        resolved: String,
    }
    let Ok(lock) = serde_json::from_slice::<Lock>(&content) else {
        return HashMap::new();
    };
    lock.features
        .into_iter()
        .map(|e| (e.reference, e.resolved))
        .collect()
}

fn write_lockfile(config_path: &Path, lock_entries: &[LockEntry]) -> anyhow::Result<()> {
    let lock_path = config_path.with_extension("lock");
    let lock_json = serde_json::json!({
        "dccVersion": env!("CARGO_PKG_VERSION"),
        "features": lock_entries,
    });
    std::fs::write(
        &lock_path,
        serde_json::to_string_pretty(&lock_json).context("failed to serialise lockfile")?,
    )
    .with_context(|| format!("failed to write lockfile `{}`", lock_path.display()))
}
