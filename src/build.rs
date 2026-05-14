use std::path::Path;

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config, docker,
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

    if config.features.is_empty() && config.container_user.is_none() {
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
    } else {
        // Build path: install features and/or create the container user.
        let context = crate::features::build_context(&config)
            .await
            .context("failed to build feature context")?;
        docker::build(image_tag.as_str(), no_cache, context)
            .await
            .with_context(|| format!("failed to build image `{}`", image_tag.as_str()))?;
    }

    Ok(())
}
