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
    script_name: &str,
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

    // Feature scripts first; devcontainer scripts override on conflict.
    let mut scripts = feature_runtime.scripts;
    scripts.extend(config.scripts);

    let cmd = scripts.get(script_name).with_context(|| {
        format!(
            "no script named `{script_name}`; available: {}",
            list_scripts(&scripts)
        )
    })?;

    let argv = vec!["/bin/sh".to_string(), "-c".to_string(), cmd.clone()];
    let status = docker::exec(
        container.as_str(),
        &config.container_user,
        CONTAINER_WORKSPACE,
        &argv,
    )
    .await?;

    std::process::exit(status.code().unwrap_or(1));
}

fn list_scripts(scripts: &IndexMap<String, String>) -> String {
    if scripts.is_empty() {
        return "(none)".to_string();
    }
    scripts.keys().cloned().collect::<Vec<_>>().join(", ")
}
