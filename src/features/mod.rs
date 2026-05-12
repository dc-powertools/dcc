use std::collections::HashSet;

use anyhow::Context as _;

use crate::config::DevcontainerConfig;

use self::{context::FeatureContext, oci::OciClient};

pub(crate) mod context;
pub(crate) mod oci;

/// Downloads all features and assembles an in-memory Docker build context tar.
/// Returns the raw (uncompressed) tar bytes for piping to `docker build -`.
pub(crate) async fn build_context(config: &DevcontainerConfig) -> anyhow::Result<Vec<u8>> {
    let mut client = OciClient::new().context("failed to initialize OCI HTTP client")?;
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut feature_contexts: Vec<FeatureContext> = Vec::new();

    for (reference, options) in &config.features {
        let downloaded = client
            .download_feature(reference, options)
            .await
            .with_context(|| format!("failed to download feature `{reference}`"))?;

        let id = context::unique_feature_id(reference, &mut seen_ids);

        feature_contexts.push(FeatureContext {
            id,
            install_sh: downloaded.install_sh,
            feature_json: downloaded.feature_json.unwrap_or_default(),
            env_vars: downloaded.env,
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
    use super::context::unique_feature_id;
    use std::collections::HashSet;

    #[test]
    fn feature_id_slug() {
        let mut seen = HashSet::new();
        let id = unique_feature_id("ghcr.io/devcontainers/features/node:1", &mut seen);
        assert_eq!(id, "ghcr-io-devcontainers-features-node-1");
    }

    #[test]
    fn feature_id_collision_handled() {
        let mut seen = HashSet::new();
        let id1 = unique_feature_id("ref-one", &mut seen);
        let id2 = unique_feature_id("ref.one", &mut seen); // same slug, different ref
        assert_eq!(id1, "ref-one");
        assert_ne!(id2, "ref-one");
        assert!(id2.starts_with("ref-one-"));
    }
}
