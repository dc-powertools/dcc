use std::collections::HashMap;

use anyhow::{bail, Context as _};
use indexmap::IndexMap;
use sha2::{Digest as _, Sha256};

pub(crate) struct DownloadedFeature {
    pub(crate) install_sh: Vec<u8>,
    pub(crate) feature_json: Option<Vec<u8>>,
    pub(crate) env: IndexMap<String, String>, // uppercased option name -> string value
}

pub(crate) struct OciClient {
    client: reqwest::Client,
    // Key: (registry, scope). Scope from WWW-Authenticate Bearer challenge.
    token_cache: HashMap<(String, String), String>,
}

struct FeatureRef {
    registry: String,   // e.g. "ghcr.io"
    repository: String, // e.g. "devcontainers/features/node"
    tag: String,        // e.g. "1"
}

impl FeatureRef {
    fn parse(s: &str) -> anyhow::Result<Self> {
        // Split on last ':' to separate tag
        let colon = s.rfind(':').ok_or_else(|| {
            anyhow::anyhow!(
                "feature reference '{}' must include a tag (e.g. 'ghcr.io/owner/repo:1')",
                s
            )
        })?;
        let tag = s[colon + 1..].to_owned();
        if tag.is_empty() {
            bail!("feature reference '{}' has an empty tag", s);
        }
        let rest = &s[..colon];
        // Split on first '/' to separate registry from repository
        let slash = rest.find('/').ok_or_else(|| {
            anyhow::anyhow!(
                "feature reference '{}' must have the form 'registry/repository:tag'",
                s
            )
        })?;
        let registry = rest[..slash].to_owned();
        let repository = rest[slash + 1..].to_owned();
        if registry.is_empty() || repository.is_empty() {
            bail!(
                "feature reference '{}' has an empty registry or repository",
                s
            );
        }
        Ok(Self {
            registry,
            repository,
            tag,
        })
    }
}

impl OciClient {
    pub(crate) fn new() -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            client,
            token_cache: HashMap::new(),
        })
    }

    pub(crate) async fn download_feature(
        &mut self,
        feature_ref: &str,
        user_options: &serde_json::Value,
    ) -> anyhow::Result<DownloadedFeature> {
        let parsed = FeatureRef::parse(feature_ref)
            .with_context(|| format!("invalid feature reference: {feature_ref}"))?;
        let manifest = self
            .fetch_manifest(&parsed)
            .await
            .with_context(|| format!("failed to fetch manifest for {feature_ref}"))?;
        let (digest, _size) = find_feature_layer(&manifest).with_context(|| {
            format!("failed to find feature layer in manifest for {feature_ref}")
        })?;
        let blob = self
            .download_blob(&parsed, &digest)
            .await
            .with_context(|| format!("failed to download blob for {feature_ref}"))?;
        let (install_sh, feature_json_bytes) = extract_feature(&blob)
            .with_context(|| format!("failed to extract feature archive for {feature_ref}"))?;
        let env = build_env(feature_json_bytes.as_deref(), user_options);
        Ok(DownloadedFeature {
            install_sh,
            feature_json: feature_json_bytes,
            env,
        })
    }

    async fn authenticate(&mut self, registry: &str, scope: &str) -> anyhow::Result<String> {
        let cache_key = (registry.to_owned(), scope.to_owned());
        if let Some(token) = self.token_cache.get(&cache_key) {
            return Ok(token.clone());
        }

        let v2_url = format!("https://{}/v2/", registry);
        let resp = self
            .client
            .get(&v2_url)
            .send()
            .await
            .with_context(|| format!("failed to contact registry {registry}"))?;

        if resp.status().is_success() {
            // No auth required
            self.token_cache.insert(cache_key, String::new());
            return Ok(String::new());
        }
        if resp.status().as_u16() != 401 {
            bail!("unexpected status {} from {}", resp.status(), v2_url);
        }

        // Parse WWW-Authenticate: Bearer realm="...",service="...",scope="..."
        let www_auth = resp
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();

        let (realm, service, challenge_scope) = parse_www_authenticate(&www_auth)
            .with_context(|| format!("failed to parse WWW-Authenticate header from {registry}"))?;

        // Use the scope from the challenge (more specific than our requested scope)
        let token_url = format!("{}?service={}&scope={}", realm, service, challenge_scope);
        let token_resp = self
            .client
            .get(&token_url)
            .send()
            .await
            .with_context(|| format!("failed to fetch token from {realm}"))?;
        if !token_resp.status().is_success() {
            bail!(
                "token endpoint returned {} for {}",
                token_resp.status(),
                registry
            );
        }
        let token_json: serde_json::Value = token_resp
            .json()
            .await
            .context("failed to parse token response")?;
        let token = token_json
            .get("token")
            .or_else(|| token_json.get("access_token"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        // Never log the token value
        tracing::debug!(registry = registry, "authenticated to OCI registry");

        self.token_cache.insert(cache_key, token.clone());
        Ok(token)
    }

    async fn fetch_manifest(&mut self, r: &FeatureRef) -> anyhow::Result<serde_json::Value> {
        let scope = format!("repository:{}:pull", r.repository);
        let token = self.authenticate(&r.registry, &scope).await?;
        let url = format!(
            "https://{}/v2/{}/manifests/{}",
            r.registry, r.repository, r.tag
        );
        let mut req = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.oci.image.manifest.v1+json");
        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        let resp = req
            .send()
            .await
            .with_context(|| format!("failed to fetch manifest from {url}"))?;
        if resp.status().as_u16() == 404 {
            bail!("feature not found at {url}");
        }
        if !resp.status().is_success() {
            bail!("manifest request returned {} for {}", resp.status(), url);
        }
        resp.json()
            .await
            .with_context(|| format!("failed to parse manifest from {url}"))
    }

    async fn download_blob(&mut self, r: &FeatureRef, digest: &str) -> anyhow::Result<Vec<u8>> {
        let scope = format!("repository:{}:pull", r.repository);
        let token = self.authenticate(&r.registry, &scope).await?;
        let url = format!(
            "https://{}/v2/{}/blobs/{}",
            r.registry, r.repository, digest
        );
        let mut req = self.client.get(&url);
        if !token.is_empty() {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        tracing::debug!(url = %url, "downloading OCI blob");
        let resp = req
            .send()
            .await
            .with_context(|| format!("failed to download blob from {url}"))?;
        if !resp.status().is_success() {
            bail!("blob download returned {} for {}", resp.status(), url);
        }
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("failed to read blob bytes from {url}"))?;
        let bytes = bytes.to_vec();

        // Mandatory digest verification
        let computed = format!("sha256:{:x}", Sha256::digest(&bytes));
        if computed != digest {
            bail!(
                "digest mismatch for {}: expected {}, got {}",
                url,
                digest,
                computed
            );
        }
        Ok(bytes)
    }
}

fn parse_www_authenticate(header: &str) -> anyhow::Result<(String, String, String)> {
    // Expects: Bearer realm="...",service="...",scope="..."
    let header = header.trim_start_matches("Bearer").trim();
    let mut realm = String::new();
    let mut service = String::new();
    let mut scope = String::new();
    for part in header.split(',') {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("realm=") {
            realm = v.trim_matches('"').to_owned();
        } else if let Some(v) = part.strip_prefix("service=") {
            service = v.trim_matches('"').to_owned();
        } else if let Some(v) = part.strip_prefix("scope=") {
            scope = v.trim_matches('"').to_owned();
        }
    }
    if realm.is_empty() {
        bail!("WWW-Authenticate header missing realm: {header}");
    }
    Ok((realm, service, scope))
}

fn find_feature_layer(manifest: &serde_json::Value) -> anyhow::Result<(String, u64)> {
    let layers = manifest["layers"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("manifest has no 'layers' array"))?;
    let feature_media_type = "application/vnd.devcontainers.layer.v1+tar";
    for layer in layers {
        if layer["mediaType"].as_str() == Some(feature_media_type) {
            let digest = layer["digest"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("layer missing 'digest' field"))?
                .to_owned();
            if !digest.starts_with("sha256:") {
                bail!("layer digest '{}' is not a sha256 digest", digest);
            }
            let size = layer["size"].as_u64().unwrap_or(0);
            return Ok((digest, size));
        }
    }
    let found: Vec<&str> = layers
        .iter()
        .filter_map(|l| l["mediaType"].as_str())
        .collect();
    bail!(
        "manifest contains no devcontainer feature layer; found media types: {:?}",
        found
    )
}

fn extract_feature(blob: &[u8]) -> anyhow::Result<(Vec<u8>, Option<Vec<u8>>)> {
    use std::io::Read;
    // Detect gzip by magic bytes
    let is_gzip = blob.len() >= 2 && blob[0] == 0x1f && blob[1] == 0x8b;
    let mut install_sh: Option<Vec<u8>> = None;
    let mut feature_json: Option<Vec<u8>> = None;

    if is_gzip {
        let mut decoder = flate2::read::GzDecoder::new(blob);
        let mut archive = tar::Archive::new(&mut decoder);
        for entry in archive.entries().context("failed to read tar archive")? {
            let mut entry = entry.context("failed to read tar entry")?;
            let path = entry.path().context("failed to get tar entry path")?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_owned();
            match name.as_str() {
                "install.sh" => {
                    let mut buf = Vec::new();
                    entry
                        .read_to_end(&mut buf)
                        .context("failed to read install.sh")?;
                    install_sh = Some(buf);
                }
                "devcontainer-feature.json" => {
                    let mut buf = Vec::new();
                    entry
                        .read_to_end(&mut buf)
                        .context("failed to read devcontainer-feature.json")?;
                    feature_json = Some(buf);
                }
                _ => {}
            }
        }
    } else {
        let mut cursor = std::io::Cursor::new(blob);
        let mut archive = tar::Archive::new(&mut cursor);
        for entry in archive.entries().context("failed to read tar archive")? {
            let mut entry = entry.context("failed to read tar entry")?;
            let path = entry.path().context("failed to get tar entry path")?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_owned();
            match name.as_str() {
                "install.sh" => {
                    let mut buf = Vec::new();
                    entry
                        .read_to_end(&mut buf)
                        .context("failed to read install.sh")?;
                    install_sh = Some(buf);
                }
                "devcontainer-feature.json" => {
                    let mut buf = Vec::new();
                    entry
                        .read_to_end(&mut buf)
                        .context("failed to read devcontainer-feature.json")?;
                    feature_json = Some(buf);
                }
                _ => {}
            }
        }
    }

    let install_sh =
        install_sh.ok_or_else(|| anyhow::anyhow!("feature archive contains no install.sh"))?;
    Ok((install_sh, feature_json))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_ref_parse_valid() {
        let r = FeatureRef::parse("ghcr.io/devcontainers/features/node:1").unwrap();
        assert_eq!(r.registry, "ghcr.io");
        assert_eq!(r.repository, "devcontainers/features/node");
        assert_eq!(r.tag, "1");
    }

    #[test]
    fn feature_ref_parse_missing_tag() {
        assert!(FeatureRef::parse("ghcr.io/devcontainers/features/node").is_err());
    }

    #[test]
    fn feature_ref_parse_empty_tag() {
        assert!(FeatureRef::parse("ghcr.io/devcontainers/features/node:").is_err());
    }

    #[test]
    fn feature_ref_parse_no_registry() {
        assert!(FeatureRef::parse("justname:1").is_err());
    }

    #[test]
    fn build_env_defaults_applied() {
        let feature_json = serde_json::json!({
            "options": {
                "version": { "type": "string", "default": "lts" }
            }
        });
        let bytes = serde_json::to_vec(&feature_json).unwrap();
        let env = build_env(Some(&bytes), &serde_json::json!({}));
        assert_eq!(env.get("VERSION"), Some(&"lts".to_string()));
    }

    #[test]
    fn build_env_user_overrides_default() {
        let feature_json = serde_json::json!({
            "options": {
                "version": { "default": "lts" }
            }
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

    #[test]
    fn find_feature_layer_correct() {
        let manifest = serde_json::json!({
            "layers": [
                { "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "digest": "sha256:abc", "size": 100 },
                { "mediaType": "application/vnd.devcontainers.layer.v1+tar", "digest": "sha256:def123", "size": 200 }
            ]
        });
        let (digest, size) = find_feature_layer(&manifest).unwrap();
        assert_eq!(digest, "sha256:def123");
        assert_eq!(size, 200);
    }

    #[test]
    fn find_feature_layer_wrong_media_type() {
        let manifest = serde_json::json!({
            "layers": [
                { "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "digest": "sha256:abc", "size": 100 }
            ]
        });
        let err = find_feature_layer(&manifest).unwrap_err();
        assert!(err
            .to_string()
            .contains("application/vnd.oci.image.layer.v1.tar+gzip"));
    }

    #[test]
    fn extract_feature_from_plain_tar() {
        // Build a minimal tar archive in memory
        let mut buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut buf);
            let content = b"#!/bin/sh\necho hello\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, "install.sh", std::io::Cursor::new(content))
                .unwrap();
            builder.finish().unwrap();
        }
        let (install_sh, feature_json) = extract_feature(&buf).unwrap();
        assert_eq!(install_sh, b"#!/bin/sh\necho hello\n");
        assert!(feature_json.is_none());
    }

    use proptest::prelude::*;
    proptest! {
        #[test]
        fn parse_never_panics(s in ".*") {
            let _ = FeatureRef::parse(&s);
        }
    }
}
