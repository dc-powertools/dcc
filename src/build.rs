use std::path::Path;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config, docker,
    features::FeatureRuntimeConfig,
    profile::{ContainerName, ProfileName},
    workspace::Workspace,
};

pub(crate) async fn build(
    workspace: &Workspace,
    profile: &ProfileName,
    config_path: &Path,
    no_cache: bool,
    strict: bool,
) -> anyhow::Result<()> {
    let cache_dir = CacheDir::new(workspace, profile);

    let config = config::load_config(config_path, workspace, &cache_dir, strict)
        .with_context(|| format!("failed to load config `{}`", config_path.display()))?;

    let container_name = ContainerName::new(workspace, profile);
    let image_tag = container_name.as_image_tag();

    // Always ensure the cache dir exists and write feature-meta.json so that
    // `dcc run` sees up-to-date runtime contributions (or an empty config when
    // there are none).
    cache_dir.ensure_exists()?;

    let runtime = if config.features.is_empty() && config.container_user.is_none() {
        // Fast path: no features and no custom user — pull and retag without a build.
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
        FeatureRuntimeConfig::default()
    } else {
        // Build path: install features and/or create the container user.
        let config_dir = config_path.parent().with_context(|| {
            format!(
                "config path `{}` has no parent directory",
                config_path.display()
            )
        })?;
        let output = crate::features::build_context(&config, config_dir)
            .await
            .context("failed to build feature context")?;
        docker::build(image_tag.as_str(), no_cache, output.context_tar)
            .await
            .with_context(|| format!("failed to build image `{}`", image_tag.as_str()))?;
        output.runtime
    };

    // Persist runtime contributions (mounts, entrypoint) for `dcc run`
    let meta_path = cache_dir.feature_meta_path();
    let meta_json =
        serde_json::to_string_pretty(&runtime).context("failed to serialize feature metadata")?;
    std::fs::write(&meta_path, meta_json)
        .with_context(|| format!("failed to write `{}`", meta_path.display()))?;

    Ok(())
}
