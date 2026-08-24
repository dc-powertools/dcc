use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context as _};
use indexmap::IndexMap;
use sha2::{Digest as _, Sha256};

/// `(install_sh, feature_json, extra_files)` extracted from a feature archive.
type ExtractedFeature = (Vec<u8>, Option<Vec<u8>>, Vec<(String, Vec<u8>, u32)>);

#[derive(Debug)]
pub(crate) struct DownloadedFeature {
    pub(crate) install_sh: Vec<u8>,
    pub(crate) feature_json: Option<Vec<u8>>,
    pub(crate) env: IndexMap<String, String>, // uppercased option name -> string value
    /// Additional files from the feature directory beyond install.sh and devcontainer-feature.json.
    /// Each entry is (filename, content, unix_mode). Empty for OCI features.
    pub(crate) extra_files: Vec<(String, Vec<u8>, u32)>,
}

pub(crate) struct OciClient {
    client: reqwest::Client,
    scheme: &'static str,
    // Key: (registry, requested repository scope).
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
            scheme: "https",
            token_cache: HashMap::new(),
        })
    }

    #[cfg(test)]
    fn new_for_http_fixture() -> anyhow::Result<Self> {
        let mut client = Self::new()?;
        client.scheme = "http";
        Ok(client)
    }

    fn registry_url(&self, registry: &str, path: &str) -> String {
        format!("{}://{registry}{path}", self.scheme)
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
        let digest = find_feature_layer(&manifest).with_context(|| {
            format!("failed to find feature layer in manifest for {feature_ref}")
        })?;
        let blob = self
            .download_blob(&parsed, &digest)
            .await
            .with_context(|| format!("failed to download blob for {feature_ref}"))?;
        let (install_sh, feature_json_bytes, extra_files) = extract_feature(&blob)
            .with_context(|| format!("failed to extract feature archive for {feature_ref}"))?;
        let env = super::build_env(feature_json_bytes.as_deref(), user_options)
            .context("failed to parse Feature metadata options")?;
        Ok(DownloadedFeature {
            install_sh,
            feature_json: feature_json_bytes,
            env,
            extra_files,
        })
    }

    async fn authenticate(&mut self, registry: &str, scope: &str) -> anyhow::Result<String> {
        let cache_key = (registry.to_owned(), scope.to_owned());
        if let Some(token) = self.token_cache.get(&cache_key) {
            return Ok(token.clone());
        }

        let v2_url = self.registry_url(registry, "/v2/");
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

        let (realm, service, _) = parse_www_authenticate(&www_auth)
            .with_context(|| format!("failed to parse WWW-Authenticate header from {registry}"))?;

        let token_url = build_token_url(&realm, &service, scope, self.scheme == "http")
            .with_context(|| format!("registry {registry} returned an invalid token realm"))?;
        let token_resp = self
            .client
            .get(token_url)
            .send()
            .await
            .with_context(|| format!("failed to fetch registry token for {registry}"))?;
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
            .ok_or_else(|| anyhow::anyhow!("token response for {registry} contained no token"))?
            .to_owned();
        if token.is_empty() {
            bail!("token response for {registry} contained an empty token");
        }

        // Never log the token value
        tracing::debug!(registry = registry, "authenticated to OCI registry");

        self.token_cache.insert(cache_key, token.clone());
        Ok(token)
    }

    async fn fetch_manifest(&mut self, r: &FeatureRef) -> anyhow::Result<serde_json::Value> {
        let scope = format!("repository:{}:pull", r.repository);
        let token = self.authenticate(&r.registry, &scope).await?;
        let url = self.registry_url(
            &r.registry,
            &format!("/v2/{}/manifests/{}", r.repository, r.tag),
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
        let url = self.registry_url(&r.registry, &format!("/v2/{}/blobs/{digest}", r.repository));
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
            .with_context(|| format!("failed to read blob bytes from {url}"))?
            .to_vec();

        verify_blob_digest(&bytes, digest)
            .with_context(|| format!("digest verification failed for {url}"))?;
        Ok(bytes)
    }
}

fn parse_www_authenticate(header: &str) -> anyhow::Result<(String, String, String)> {
    let (scheme, parameters) = header
        .trim()
        .split_once(char::is_whitespace)
        .ok_or_else(|| anyhow::anyhow!("invalid WWW-Authenticate challenge"))?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        bail!("unsupported WWW-Authenticate scheme `{scheme}`");
    }
    let mut realm = String::new();
    let mut service = String::new();
    let mut scope = String::new();
    for part in split_quoted(parameters, ',') {
        let part = part.trim();
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').to_owned();
        if key.trim().eq_ignore_ascii_case("realm") {
            realm = value;
        } else if key.trim().eq_ignore_ascii_case("service") {
            service = value;
        } else if key.trim().eq_ignore_ascii_case("scope") {
            scope = value;
        }
    }
    if realm.is_empty() {
        bail!("WWW-Authenticate Bearer challenge is missing a realm");
    }
    Ok((realm, service, scope))
}

fn split_quoted(input: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in input.char_indices() {
        if escaped {
            escaped = false;
        } else if ch == '\\' && quoted {
            escaped = true;
        } else if ch == '"' {
            quoted = !quoted;
        } else if ch == delimiter && !quoted {
            parts.push(&input[start..index]);
            start = index + ch.len_utf8();
        }
    }
    parts.push(&input[start..]);
    parts
}

fn build_token_url(
    realm: &str,
    service: &str,
    scope: &str,
    allow_insecure_http: bool,
) -> anyhow::Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(realm).context("token realm is not a valid URL")?;
    if url.scheme() != "https" && !(allow_insecure_http && url.scheme() == "http") {
        bail!("token realm must use HTTPS");
    }
    {
        let mut query = url.query_pairs_mut();
        if !service.is_empty() {
            query.append_pair("service", service);
        }
        query.append_pair("scope", scope);
    }
    Ok(url)
}

fn verify_blob_digest(bytes: &[u8], digest: &str) -> anyhow::Result<()> {
    let encoded = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("unsupported blob digest algorithm"))?;
    if encoded.len() != 64 || !encoded.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid sha256 blob digest");
    }

    let computed = format!("sha256:{:x}", Sha256::digest(bytes));
    if computed != digest.to_ascii_lowercase() {
        bail!("digest mismatch: expected {digest}, got {computed}");
    }
    Ok(())
}

fn find_feature_layer(manifest: &serde_json::Value) -> anyhow::Result<String> {
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
            return Ok(digest);
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

fn extract_feature(blob: &[u8]) -> anyhow::Result<ExtractedFeature> {
    // Detect gzip by magic bytes
    let is_gzip = blob.len() >= 2 && blob[0] == 0x1f && blob[1] == 0x8b;
    if is_gzip {
        let mut decoder = flate2::read::GzDecoder::new(blob);
        extract_from_tar(tar::Archive::new(&mut decoder))
    } else {
        extract_from_tar(tar::Archive::new(std::io::Cursor::new(blob)))
    }
}

fn extract_from_tar<R: std::io::Read>(
    mut archive: tar::Archive<R>,
) -> anyhow::Result<ExtractedFeature> {
    use std::io::Read as _;

    // Per the devcontainer spec, a feature tarball contains the *contents* of the
    // feature directory — install.sh and devcontainer-feature.json are at the root,
    // and any helper files or subdirectories (e.g. library_scripts/common.sh) are
    // also relative to that root. We strip the leading "./" that some tar tools
    // emit and match paths exactly; no prefix detection is needed.
    let mut install_sh: Option<Vec<u8>> = None;
    let mut feature_json: Option<Vec<u8>> = None;
    let mut extra_files: Vec<(String, Vec<u8>, u32)> = Vec::new();

    for entry in archive.entries().context("failed to read tar archive")? {
        let mut entry = entry.context("failed to read tar entry")?;
        let path = safe_archive_path(&entry.path().context("failed to get tar entry path")?)?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            continue;
        }
        if !entry_type.is_file() {
            bail!("feature archive entry {path:?} has an unsupported type");
        }
        let relative = path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("feature archive path is not valid UTF-8"))?;

        let mode = entry.header().mode().unwrap_or(0o644);
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .with_context(|| format!("failed to read `{relative}`"))?;

        match relative {
            "install.sh" => install_sh = Some(buf),
            "devcontainer-feature.json" => feature_json = Some(buf),
            name if !name.is_empty() => extra_files.push((name.to_owned(), buf, mode)),
            _ => {}
        }
    }

    let install_sh =
        install_sh.ok_or_else(|| anyhow::anyhow!("feature archive contains no install.sh"))?;
    Ok((install_sh, feature_json, extra_files))
}

fn safe_archive_path(path: &Path) -> anyhow::Result<PathBuf> {
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => safe.push(segment),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("unsafe path {path:?} in feature archive");
            }
        }
    }
    if safe.as_os_str().is_empty() {
        bail!("feature archive contains an empty path");
    }
    Ok(safe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;

    #[derive(Clone)]
    struct TestResponse {
        status: &'static str,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl TestResponse {
        fn new(status: &'static str, body: impl Into<Vec<u8>>) -> Self {
            Self {
                status,
                headers: Vec::new(),
                body: body.into(),
            }
        }

        fn header(mut self, name: &str, value: impl Into<String>) -> Self {
            self.headers.push((name.to_string(), value.into()));
            self
        }
    }

    async fn start_scripted_server(
        responses: impl FnOnce(&str) -> Vec<TestResponse>,
    ) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let responses = Arc::new(Mutex::new(VecDeque::from(responses(&origin))));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = Arc::clone(&requests);
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut request = Vec::new();
                let mut chunk = [0_u8; 4096];
                loop {
                    let read = socket.read(&mut chunk).await.unwrap();
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8(request).unwrap();
                server_requests.lock().unwrap().push(request);
                let response = responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("scripted server received an unexpected request");
                let mut head = format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                    response.status,
                    response.body.len()
                );
                for (name, value) in response.headers {
                    head.push_str(&format!("{name}: {value}\r\n"));
                }
                head.push_str("\r\n");
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(&response.body).await.unwrap();
            }
        });
        (origin, requests, server)
    }

    fn request_path(request: &str) -> &str {
        request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap()
    }

    fn feature_tar(metadata: Option<&[u8]>) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut builder = tar::Builder::new(&mut bytes);
        tar_entry(
            &mut builder,
            "install.sh",
            b"#!/bin/sh\nprintf installed\n",
            0o755,
        );
        if let Some(metadata) = metadata {
            tar_entry(&mut builder, "devcontainer-feature.json", metadata, 0o644);
        }
        builder.finish().unwrap();
        drop(builder);
        bytes
    }

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
    fn find_feature_layer_correct() {
        let manifest = serde_json::json!({
            "layers": [
                { "mediaType": "application/vnd.oci.image.layer.v1.tar+gzip", "digest": "sha256:abc", "size": 100 },
                { "mediaType": "application/vnd.devcontainers.layer.v1+tar", "digest": "sha256:def123", "size": 200 }
            ]
        });
        let digest = find_feature_layer(&manifest).unwrap();
        assert_eq!(digest, "sha256:def123");
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
    fn bearer_challenge_allows_reordered_and_optional_parameters() {
        let (realm, service, scope) = parse_www_authenticate(
            r#"bearer scope="repo:a:pull",extra="a,b",realm="https://auth.example/token""#,
        )
        .unwrap();
        assert_eq!(realm, "https://auth.example/token");
        assert!(service.is_empty());
        assert_eq!(scope, "repo:a:pull");
    }

    #[test]
    fn bearer_challenge_rejects_missing_realm_and_other_schemes() {
        assert!(parse_www_authenticate(r#"Bearer service="registry""#).is_err());
        assert!(parse_www_authenticate(r#"Basic realm="registry""#).is_err());
    }

    #[test]
    fn production_token_url_requires_https_and_encodes_parameters() {
        let error = build_token_url(
            "http://auth.example/token",
            "fixture",
            "repository:owner/feature:pull",
            false,
        )
        .unwrap_err();
        assert!(error.to_string().contains("must use HTTPS"));

        let url = build_token_url(
            "https://auth.example/token",
            "fixture service",
            "repository:owner/feature:pull",
            false,
        )
        .unwrap();
        assert_eq!(url.scheme(), "https");
        assert!(url.as_str().contains("service=fixture+service"));
        assert!(url
            .as_str()
            .contains("scope=repository%3Aowner%2Ffeature%3Apull"));
    }

    #[test]
    fn blob_digest_accepts_exact_content_and_rejects_one_byte_mutation() {
        let content = b"trusted feature blob";
        let digest = format!("sha256:{:x}", Sha256::digest(content));
        verify_blob_digest(content, &digest).unwrap();

        let mut mutated = content.to_vec();
        mutated[0] ^= 1;
        let error = verify_blob_digest(&mutated, &digest).unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn blob_digest_rejects_malformed_digest_values() {
        for digest in ["sha512:abc", "sha256:abc", "sha256:not-hex"] {
            assert!(verify_blob_digest(b"content", digest).is_err());
        }
    }

    fn tar_entry(builder: &mut tar::Builder<&mut Vec<u8>>, path: &str, content: &[u8], mode: u32) {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        builder
            .append_data(&mut header, path, std::io::Cursor::new(content))
            .unwrap();
    }

    fn raw_tar_entry(
        builder: &mut tar::Builder<&mut Vec<u8>>,
        path: &[u8],
        entry_type: tar::EntryType,
    ) {
        assert!(path.len() < 100);
        let mut header = tar::Header::new_gnu();
        header.as_mut_bytes()[..path.len()].copy_from_slice(path);
        header.set_size(0);
        header.set_mode(0o644);
        header.set_entry_type(entry_type);
        if matches!(entry_type, tar::EntryType::Link | tar::EntryType::Symlink) {
            header.set_link_name("../outside").unwrap();
        }
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
    }

    #[test]
    fn extract_feature_from_plain_tar() {
        let mut buf = Vec::new();
        let mut builder = tar::Builder::new(&mut buf);
        tar_entry(
            &mut builder,
            "install.sh",
            b"#!/bin/sh\necho hello\n",
            0o755,
        );
        builder.finish().unwrap();
        drop(builder);

        let (install_sh, feature_json, extra_files) = extract_feature(&buf).unwrap();
        assert_eq!(install_sh, b"#!/bin/sh\necho hello\n");
        assert!(feature_json.is_none());
        assert!(extra_files.is_empty());
    }

    #[test]
    fn extract_feature_collects_extra_files() {
        let mut buf = Vec::new();
        let mut builder = tar::Builder::new(&mut buf);
        tar_entry(
            &mut builder,
            "install.sh",
            b"#!/bin/sh\n./helper.sh\n",
            0o755,
        );
        tar_entry(&mut builder, "helper.sh", b"#!/bin/sh\necho hi\n", 0o755);
        builder.finish().unwrap();
        drop(builder);

        let (_, _, extra_files) = extract_feature(&buf).unwrap();
        assert_eq!(extra_files.len(), 1);
        assert_eq!(extra_files[0].0, "helper.sh");
        assert_eq!(extra_files[0].1, b"#!/bin/sh\necho hi\n");
    }

    #[test]
    fn extract_feature_preserves_nested_paths() {
        // Subdirectory paths must be preserved so install.sh can source them correctly
        let mut buf = Vec::new();
        let mut builder = tar::Builder::new(&mut buf);
        tar_entry(&mut builder, "install.sh", b"#!/bin/sh\n", 0o755);
        tar_entry(
            &mut builder,
            "library_scripts/common.sh",
            b"#!/bin/sh\necho common\n",
            0o755,
        );
        builder.finish().unwrap();
        drop(builder);

        let (_, _, extra_files) = extract_feature(&buf).unwrap();
        assert_eq!(extra_files.len(), 1);
        assert_eq!(extra_files[0].0, "library_scripts/common.sh");
    }

    #[test]
    fn extract_feature_rejects_parent_and_absolute_paths() {
        for unsafe_path in [
            b"../escape".as_slice(),
            b"/absolute".as_slice(),
            b"./../../escape",
        ] {
            let mut buf = Vec::new();
            let mut builder = tar::Builder::new(&mut buf);
            tar_entry(&mut builder, "install.sh", b"#!/bin/sh\n", 0o755);
            raw_tar_entry(&mut builder, unsafe_path, tar::EntryType::Regular);
            builder.finish().unwrap();
            drop(builder);

            let error = extract_feature(&buf).unwrap_err();
            assert!(
                error.to_string().contains("unsafe path"),
                "unexpected error for {}: {error}",
                String::from_utf8_lossy(unsafe_path)
            );
        }
    }

    #[test]
    fn extract_feature_rejects_links_and_special_entries() {
        for entry_type in [
            tar::EntryType::Symlink,
            tar::EntryType::Link,
            tar::EntryType::Fifo,
            tar::EntryType::Char,
        ] {
            let mut buf = Vec::new();
            let mut builder = tar::Builder::new(&mut buf);
            tar_entry(&mut builder, "install.sh", b"#!/bin/sh\n", 0o755);
            raw_tar_entry(&mut builder, b"unsafe-entry", entry_type);
            builder.finish().unwrap();
            drop(builder);

            let error = extract_feature(&buf).unwrap_err();
            assert!(error.to_string().contains("unsupported type"));
        }
    }

    #[tokio::test]
    async fn registry_flow_authenticates_once_reuses_token_and_verifies_blob() {
        let blob = feature_tar(Some(
            br#"{"id":"fixture","options":{"version":{"default":"1"}}}"#,
        ));
        let digest = format!("sha256:{:x}", Sha256::digest(&blob));
        let manifest = serde_json::to_vec(&serde_json::json!({
            "layers": [{
                "mediaType": "application/vnd.devcontainers.layer.v1+tar",
                "digest": digest,
                "size": blob.len()
            }]
        }))
        .unwrap();
        let (origin, requests, server) = start_scripted_server(|origin| {
            vec![
                TestResponse::new("401 Unauthorized", Vec::new()).header(
                    "WWW-Authenticate",
                    format!(
                        "Bearer scope=\"ignored\", realm=\"{origin}/token\", service=\"fixture\""
                    ),
                ),
                TestResponse::new("200 OK", br#"{"token":"fixture-token"}"#.as_slice()),
                TestResponse::new("200 OK", manifest).header("Content-Type", "application/json"),
                TestResponse::new("200 OK", blob),
            ]
        })
        .await;
        let registry = origin.strip_prefix("http://").unwrap();
        let mut client = OciClient::new_for_http_fixture().unwrap();

        let feature = client
            .download_feature(
                &format!("{registry}/owner/feature:1"),
                &serde_json::json!({}),
            )
            .await
            .unwrap();
        assert_eq!(feature.install_sh, b"#!/bin/sh\nprintf installed\n");
        assert_eq!(feature.env.get("VERSION").map(String::as_str), Some("1"));

        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(request_path(&requests[0]), "/v2/");
        assert!(request_path(&requests[1]).starts_with("/token?"));
        assert!(request_path(&requests[1]).contains("service=fixture"));
        assert!(request_path(&requests[1]).contains("scope=repository%3Aowner%2Ffeature%3Apull"));
        assert!(requests[2]
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-token"));
        assert!(requests[3]
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-token"));
        assert_eq!(
            requests
                .iter()
                .filter(|r| request_path(r) == "/v2/")
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|r| request_path(r).starts_with("/token?"))
                .count(),
            1
        );
        drop(requests);
        server.abort();
    }

    #[tokio::test]
    async fn registry_auth_failure_is_sanitized() {
        const SECRET_BODY: &str = "sensitive-token-endpoint-body";
        let (origin, _requests, server) = start_scripted_server(|origin| {
            vec![
                TestResponse::new("401 Unauthorized", Vec::new()).header(
                    "WWW-Authenticate",
                    format!("Bearer realm=\"{origin}/token\",service=\"fixture\""),
                ),
                TestResponse::new("403 Forbidden", SECRET_BODY),
            ]
        })
        .await;
        let registry = origin.strip_prefix("http://").unwrap();
        let error = OciClient::new_for_http_fixture()
            .unwrap()
            .download_feature(
                &format!("{registry}/owner/feature:1"),
                &serde_json::json!({}),
            )
            .await
            .unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("token endpoint returned 403"));
        assert!(!error.contains(SECRET_BODY));
        server.abort();
    }

    #[tokio::test]
    async fn registry_manifest_and_blob_failures_are_sanitized() {
        const SECRET_BODY: &str = "sensitive-registry-response";
        let (origin, _requests, server) = start_scripted_server(|_| {
            vec![
                TestResponse::new("200 OK", Vec::new()),
                TestResponse::new("503 Service Unavailable", SECRET_BODY),
            ]
        })
        .await;
        let registry = origin.strip_prefix("http://").unwrap();
        let mut client = OciClient::new_for_http_fixture().unwrap();
        let error = client
            .download_feature(
                &format!("{registry}/owner/feature:1"),
                &serde_json::json!({}),
            )
            .await
            .unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("manifest request returned 503"));
        assert!(!error.contains(SECRET_BODY));
        server.abort();

        let blob = feature_tar(None);
        let digest = format!("sha256:{:x}", Sha256::digest(&blob));
        let manifest = serde_json::to_vec(&serde_json::json!({
            "layers": [{
                "mediaType": "application/vnd.devcontainers.layer.v1+tar",
                "digest": digest,
                "size": blob.len()
            }]
        }))
        .unwrap();
        let (origin, _requests, server) = start_scripted_server(|_| {
            vec![
                TestResponse::new("200 OK", Vec::new()),
                TestResponse::new("200 OK", manifest).header("Content-Type", "application/json"),
                TestResponse::new("500 Internal Server Error", SECRET_BODY),
            ]
        })
        .await;
        let registry = origin.strip_prefix("http://").unwrap();
        let error = OciClient::new_for_http_fixture()
            .unwrap()
            .download_feature(
                &format!("{registry}/owner/feature:1"),
                &serde_json::json!({}),
            )
            .await
            .unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("blob download returned 500"));
        assert!(!error.contains(SECRET_BODY));
        server.abort();
    }

    use proptest::prelude::*;
    proptest! {
        #[test]
        fn valid_feature_references_round_trip(
            registry in "[a-z]{1,8}(\\.[a-z]{1,8})?",
            repository in "[a-z]{1,8}(/[a-z]{1,8}){0,3}",
            tag in "[a-z0-9]{1,8}",
        ) {
            let reference = format!("{registry}/{repository}:{tag}");
            let parsed = FeatureRef::parse(&reference).unwrap();
            prop_assert_eq!(parsed.registry, registry);
            prop_assert_eq!(parsed.repository, repository);
            prop_assert_eq!(parsed.tag, tag);
        }
    }
}
