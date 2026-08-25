use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context as _};
use indexmap::IndexMap;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, LOCATION};
use sha2::{Digest as _, Sha256};

use crate::config::registry_ca::{RegistryAuthority, RegistryCaBundle};

const MAX_REDIRECTS: usize = 10;

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
    public_client: reqwest::Client,
    registry_cas: BTreeMap<RegistryAuthority, RegistryCaBundle>,
    registry_clients: HashMap<RegistryAuthority, reqwest::Client>,
    #[cfg(test)]
    baseline_roots: Vec<reqwest::Certificate>,
    allow_insecure_http: bool,
    // Key: (registry, requested repository scope).
    token_cache: HashMap<(RegistryAuthority, String), String>,
}

#[derive(Debug)]
struct FeatureRef {
    registry: RegistryAuthority, // e.g. "ghcr.io"
    repository: String,          // e.g. "devcontainers/features/node"
    tag: String,                 // e.g. "1"
}

impl FeatureRef {
    fn parse(s: &str) -> anyhow::Result<Self> {
        // Split on last ':' to separate tag
        let colon = s.rfind(':').ok_or_else(|| {
            anyhow::anyhow!("feature reference must include a tag (e.g. 'ghcr.io/owner/repo:1')")
        })?;
        let tag = s[colon + 1..].to_owned();
        if tag.is_empty() {
            bail!("feature reference has an empty tag");
        }
        let rest = &s[..colon];
        // Split on first '/' to separate registry from repository
        let slash = rest.find('/').ok_or_else(|| {
            anyhow::anyhow!("feature reference must have the form 'registry/repository:tag'")
        })?;
        let registry = RegistryAuthority::parse(&rest[..slash])
            .context("feature reference has an invalid registry authority")?;
        let repository = rest[slash + 1..].to_owned();
        if repository.is_empty() {
            bail!("feature reference has an empty repository");
        }
        Ok(Self {
            registry,
            repository,
            tag,
        })
    }
}

impl OciClient {
    pub(crate) fn new(
        registry_cas: &BTreeMap<RegistryAuthority, RegistryCaBundle>,
    ) -> anyhow::Result<Self> {
        let public_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self {
            public_client,
            registry_cas: registry_cas.clone(),
            registry_clients: HashMap::new(),
            #[cfg(test)]
            baseline_roots: Vec::new(),
            allow_insecure_http: false,
            token_cache: HashMap::new(),
        })
    }

    #[cfg(test)]
    fn new_with_baseline_roots(
        registry_cas: &BTreeMap<RegistryAuthority, RegistryCaBundle>,
        baseline_roots: Vec<reqwest::Certificate>,
    ) -> anyhow::Result<Self> {
        let mut builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
        for certificate in &baseline_roots {
            builder = builder.add_root_certificate(certificate.clone());
        }
        let public_client = builder.build().context("failed to build HTTP client")?;
        Ok(Self {
            public_client,
            registry_cas: registry_cas.clone(),
            registry_clients: HashMap::new(),
            baseline_roots,
            allow_insecure_http: false,
            token_cache: HashMap::new(),
        })
    }

    #[cfg(test)]
    fn new_for_http_fixture() -> anyhow::Result<Self> {
        let mut client = Self::new(&BTreeMap::new())?;
        client.allow_insecure_http = true;
        Ok(client)
    }

    fn registry_url(
        &self,
        registry: &RegistryAuthority,
        path: &str,
    ) -> anyhow::Result<reqwest::Url> {
        let scheme = if self.allow_insecure_http {
            "http"
        } else {
            "https"
        };
        reqwest::Url::parse(&format!("{scheme}://{registry}{path}"))
            .context("failed to construct registry URL")
    }

    fn client_for_url(&mut self, url: &reqwest::Url) -> anyhow::Result<reqwest::Client> {
        validate_request_target(url, self.allow_insecure_http)?;
        if self.allow_insecure_http && url.scheme() == "http" {
            return Ok(self.public_client.clone());
        }
        let authority = RegistryAuthority::from_url(url)
            .context("request URL has an invalid registry authority")?;
        let Some(bundle) = self.registry_cas.get(&authority) else {
            return Ok(self.public_client.clone());
        };
        if let Some(client) = self.registry_clients.get(&authority) {
            return Ok(client.clone());
        }
        let mut builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
        #[cfg(test)]
        for certificate in &self.baseline_roots {
            builder = builder.add_root_certificate(certificate.clone());
        }
        for certificate in &bundle.certificates {
            builder = builder.add_root_certificate(certificate.clone());
        }
        let client = builder
            .build()
            .with_context(|| format!("failed to build HTTPS client for registry `{authority}`"))?;
        self.registry_clients.insert(authority, client.clone());
        Ok(client)
    }

    async fn get_following_redirects(
        &mut self,
        initial_url: reqwest::Url,
        mut headers: HeaderMap,
        operation: &str,
    ) -> anyhow::Result<reqwest::Response> {
        let mut current = initial_url;
        let mut visited = HashSet::new();
        visited.insert(current.as_str().to_owned());

        for redirects in 0..=MAX_REDIRECTS {
            let authority = sanitized_authority(&current);
            let client = self
                .client_for_url(&current)
                .with_context(|| format!("{operation} rejected target authority `{authority}`"))?;
            let response = client
                .get(current.clone())
                .headers(headers.clone())
                .send()
                .await
                .map_err(|error| anyhow::Error::new(error.without_url()))
                .with_context(|| format!("{operation} failed for authority `{authority}`"))?;

            if !is_redirect_status(response.status()) {
                return Ok(response);
            }
            if redirects == MAX_REDIRECTS {
                bail!("{operation} exceeded the maximum of {MAX_REDIRECTS} redirects");
            }

            let location = response
                .headers()
                .get(LOCATION)
                .with_context(|| {
                    format!("{operation} redirect from authority `{authority}` is missing Location")
                })?
                .to_str()
                .with_context(|| {
                    format!(
                        "{operation} redirect from authority `{authority}` has an invalid Location"
                    )
                })?;
            let next = current.join(location).with_context(|| {
                format!("{operation} redirect from authority `{authority}` has an invalid Location")
            })?;
            validate_request_target(&next, self.allow_insecure_http).with_context(|| {
                format!("{operation} redirect from authority `{authority}` was rejected")
            })?;
            if !same_origin(&current, &next) {
                headers.remove(AUTHORIZATION);
            }
            if !visited.insert(next.as_str().to_owned()) {
                bail!(
                    "{operation} encountered a redirect loop at authority `{}`",
                    sanitized_authority(&next)
                );
            }
            current = next;
        }

        bail!("{operation} exceeded the maximum of {MAX_REDIRECTS} redirects")
    }

    pub(crate) async fn download_feature(
        &mut self,
        feature_ref: &str,
        user_options: &serde_json::Value,
    ) -> anyhow::Result<DownloadedFeature> {
        let parsed = FeatureRef::parse(feature_ref).context("invalid feature reference")?;
        let manifest = self.fetch_manifest(&parsed).await.with_context(|| {
            format!("failed to fetch manifest from registry {}", parsed.registry)
        })?;
        let digest = find_feature_layer(&manifest).context("failed to find feature layer")?;
        let blob = self
            .download_blob(&parsed, &digest)
            .await
            .with_context(|| {
                format!("failed to download blob from registry {}", parsed.registry)
            })?;
        let (install_sh, feature_json_bytes, extra_files) =
            extract_feature(&blob).context("failed to extract feature archive")?;
        let env = super::build_env(feature_json_bytes.as_deref(), user_options)
            .context("failed to parse Feature metadata options")?;
        Ok(DownloadedFeature {
            install_sh,
            feature_json: feature_json_bytes,
            env,
            extra_files,
        })
    }

    async fn authenticate(
        &mut self,
        registry: &RegistryAuthority,
        scope: &str,
    ) -> anyhow::Result<String> {
        let cache_key = (registry.clone(), scope.to_owned());
        if let Some(token) = self.token_cache.get(&cache_key) {
            return Ok(token.clone());
        }

        let v2_url = self.registry_url(registry, "/v2/")?;
        let resp = self
            .get_following_redirects(v2_url, HeaderMap::new(), "registry contact")
            .await
            .with_context(|| format!("failed to contact registry {registry}"))?;

        if resp.status().is_success() {
            // No auth required
            self.token_cache.insert(cache_key, String::new());
            return Ok(String::new());
        }
        if resp.status().as_u16() != 401 {
            bail!(
                "registry {registry} returned unexpected status {}",
                resp.status()
            );
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

        let token_url = build_token_url(&realm, &service, scope, self.allow_insecure_http)
            .with_context(|| format!("registry {registry} returned an invalid token realm"))?;
        let token_resp = self
            .get_following_redirects(token_url, HeaderMap::new(), "registry token request")
            .await
            .with_context(|| format!("failed to fetch registry token for {registry}"))?;
        if !token_resp.status().is_success() {
            bail!(
                "token endpoint returned {} for {}",
                token_resp.status(),
                registry
            );
        }
        let token_bytes = response_bytes(token_resp, "registry token response").await?;
        let token_json: serde_json::Value =
            serde_json::from_slice(&token_bytes).context("failed to parse token response")?;
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
        tracing::debug!(registry = %registry, "authenticated to OCI registry");

        self.token_cache.insert(cache_key, token.clone());
        Ok(token)
    }

    async fn fetch_manifest(&mut self, r: &FeatureRef) -> anyhow::Result<serde_json::Value> {
        let scope = format!("repository:{}:pull", r.repository);
        let token = self.authenticate(&r.registry, &scope).await?;
        let url = self.registry_url(
            &r.registry,
            &format!("/v2/{}/manifests/{}", r.repository, r.tag),
        )?;
        let headers = registry_headers(&token, Some("application/vnd.oci.image.manifest.v1+json"))?;
        let resp = self
            .get_following_redirects(url, headers, "registry manifest request")
            .await
            .with_context(|| format!("failed to fetch manifest from registry {}", r.registry))?;
        if resp.status().as_u16() == 404 {
            bail!("feature not found at registry {}", r.registry);
        }
        if !resp.status().is_success() {
            bail!(
                "manifest request returned {} for registry {}",
                resp.status(),
                r.registry
            );
        }
        let bytes = response_bytes(resp, "registry manifest response").await?;
        serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse manifest from registry {}", r.registry))
    }

    async fn download_blob(&mut self, r: &FeatureRef, digest: &str) -> anyhow::Result<Vec<u8>> {
        let scope = format!("repository:{}:pull", r.repository);
        let token = self.authenticate(&r.registry, &scope).await?;
        let url =
            self.registry_url(&r.registry, &format!("/v2/{}/blobs/{digest}", r.repository))?;
        let headers = registry_headers(&token, None)?;
        tracing::debug!(registry = %r.registry, "downloading OCI blob");
        let resp = self
            .get_following_redirects(url, headers, "registry blob request")
            .await
            .with_context(|| format!("failed to download blob from registry {}", r.registry))?;
        if !resp.status().is_success() {
            bail!(
                "blob download returned {} for registry {}",
                resp.status(),
                r.registry
            );
        }
        let bytes = response_bytes(resp, "registry blob response").await?;

        verify_blob_digest(&bytes, digest)
            .with_context(|| format!("digest verification failed for registry {}", r.registry))?;
        Ok(bytes)
    }
}

fn registry_headers(token: &str, accept: Option<&str>) -> anyhow::Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    if let Some(accept) = accept {
        headers.insert(
            ACCEPT,
            HeaderValue::from_str(accept).context("invalid Accept header")?,
        );
    }
    if !token.is_empty() {
        let value = HeaderValue::from_str(&format!("Bearer {token}"))
            .context("registry returned a token that cannot be used as an HTTP header")?;
        headers.insert(AUTHORIZATION, value);
    }
    Ok(headers)
}

async fn response_bytes(response: reqwest::Response, operation: &str) -> anyhow::Result<Vec<u8>> {
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| anyhow::anyhow!("failed to read {operation}: {}", error.without_url()))
}

fn is_redirect_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
}

fn same_origin(left: &reqwest::Url, right: &reqwest::Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn validate_request_target(url: &reqwest::Url, allow_insecure_http: bool) -> anyhow::Result<()> {
    if url.scheme() != "https" && !(allow_insecure_http && url.scheme() == "http") {
        bail!("OCI requests must use HTTPS");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("OCI request URLs must not contain user information");
    }
    if url.host().is_none() {
        bail!("OCI request URL is missing a host");
    }
    Ok(())
}

fn sanitized_authority(url: &reqwest::Url) -> String {
    let host = url
        .host_str()
        .map(|host| host.trim_start_matches('[').trim_end_matches(']'));
    match (host, url.port()) {
        (Some(host), Some(port)) if host.contains(':') => format!("[{host}]:{port}"),
        (Some(host), None) if host.contains(':') => format!("[{host}]"),
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_string(),
        (None, _) => "<invalid>".to_string(),
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
        if path.as_os_str().is_empty() {
            if entry_type.is_dir() {
                continue;
            }
            bail!("feature archive contains an empty path");
        }
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
    Ok(safe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, Issuer, KeyPair};
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::TcpListener;
    use tokio_rustls::TlsAcceptor;

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

    fn test_ca() -> CertifiedIssuer<'static, KeyPair> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap()).unwrap()
    }

    fn tls_server_config(
        issuer: &Issuer<'_, impl rcgen::SigningKey>,
        subject_alt_names: Vec<String>,
    ) -> rustls::ServerConfig {
        tls_server_config_with_params(issuer, CertificateParams::new(subject_alt_names).unwrap())
    }

    fn tls_server_config_with_params(
        issuer: &Issuer<'_, impl rcgen::SigningKey>,
        params: CertificateParams,
    ) -> rustls::ServerConfig {
        let key = KeyPair::generate().unwrap();
        let certificate = params.signed_by(&key, issuer).unwrap();
        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![certificate.der().clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
            )
            .unwrap()
    }

    async fn start_tls_scripted_server(
        config: rustls::ServerConfig,
        responses: impl FnOnce(&str) -> Vec<TestResponse>,
    ) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("https://{}", listener.local_addr().unwrap());
        let responses = Arc::new(Mutex::new(VecDeque::from(responses(&origin))));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = Arc::clone(&requests);
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let server = tokio::spawn(async move {
            loop {
                let Ok((socket, _)) = listener.accept().await else {
                    break;
                };
                let Ok(mut socket) = acceptor.accept(socket).await else {
                    continue;
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
                    .expect("TLS scripted server received an unexpected request");
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

    fn ca_bundle(issuer: &CertifiedIssuer<'_, KeyPair>) -> RegistryCaBundle {
        RegistryCaBundle {
            certificates: vec![reqwest::Certificate::from_der(issuer.der()).unwrap()],
        }
    }

    fn ca_bundle_many<'a>(
        issuers: impl IntoIterator<Item = &'a CertifiedIssuer<'static, KeyPair>>,
    ) -> RegistryCaBundle {
        RegistryCaBundle {
            certificates: issuers
                .into_iter()
                .map(|issuer| reqwest::Certificate::from_der(issuer.der()).unwrap())
                .collect(),
        }
    }

    fn error_chain_text(error: &anyhow::Error) -> String {
        error
            .chain()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(": ")
    }

    fn assert_chain_contains(error: &anyhow::Error, expected: &[&str]) {
        let chain = error_chain_text(error).to_ascii_lowercase();
        assert!(
            expected.iter().any(|needle| chain.contains(needle)),
            "error chain did not contain one of {expected:?}: {chain}"
        );
    }

    fn configured_client(
        entries: impl IntoIterator<Item = (String, RegistryCaBundle)>,
    ) -> OciClient {
        let entries = entries
            .into_iter()
            .map(|(authority, bundle)| (RegistryAuthority::parse(&authority).unwrap(), bundle))
            .collect();
        OciClient::new(&entries).unwrap()
    }

    fn origin_authority(origin: &str) -> String {
        origin
            .strip_prefix("https://")
            .or_else(|| origin.strip_prefix("http://"))
            .unwrap()
            .to_string()
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
        raw_tar_entry(&mut builder, b"./", tar::EntryType::Directory);
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

    fn feature_manifest(blob: &[u8]) -> Vec<u8> {
        let digest = format!("sha256:{:x}", Sha256::digest(blob));
        serde_json::to_vec(&serde_json::json!({
            "layers": [{
                "mediaType": "application/vnd.devcontainers.layer.v1+tar",
                "digest": digest,
                "size": blob.len()
            }]
        }))
        .unwrap()
    }

    #[test]
    fn feature_ref_parse_valid() {
        let r = FeatureRef::parse("ghcr.io/devcontainers/features/node:1").unwrap();
        assert_eq!(r.registry.to_string(), "ghcr.io");
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
    fn feature_reference_errors_do_not_echo_user_information_or_query_data() {
        const SECRET: &str = "sentinel-feature-reference-secret";
        let error = FeatureRef::parse(&format!(
            "user:{SECRET}@registry.example?query={SECRET}/owner/feature:1"
        ))
        .unwrap_err();
        assert!(!format!("{error:#}").contains(SECRET));
    }

    #[test]
    fn ipv6_authority_diagnostics_have_one_bracket_pair() {
        let url = reqwest::Url::parse("https://[2001:db8::1]:5443/v2/").unwrap();
        assert_eq!(sanitized_authority(&url), "[2001:db8::1]:5443");
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

    #[tokio::test]
    async fn manual_redirects_preserve_same_origin_authorization() {
        let (origin, requests, server) = start_scripted_server(|_| {
            vec![
                TestResponse::new("302 Found", Vec::new()).header("Location", "/target"),
                TestResponse::new("200 OK", b"done".as_slice()),
            ]
        })
        .await;
        let mut client = OciClient::new_for_http_fixture().unwrap();
        let headers = registry_headers("same-origin-token", None).unwrap();
        let response = client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{origin}/start")).unwrap(),
                headers,
                "redirect fixture",
            )
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer same-origin-token"));
        assert!(requests[1]
            .to_ascii_lowercase()
            .contains("authorization: bearer same-origin-token"));
        drop(requests);
        server.abort();
    }

    #[tokio::test]
    async fn manual_redirects_strip_cross_origin_authorization() {
        let (target_origin, target_requests, target_server) =
            start_scripted_server(|_| vec![TestResponse::new("200 OK", b"done".as_slice())]).await;
        let (source_origin, source_requests, source_server) = start_scripted_server(|_| {
            vec![TestResponse::new("307 Temporary Redirect", Vec::new())
                .header("Location", format!("{target_origin}/target"))]
        })
        .await;
        let mut client = OciClient::new_for_http_fixture().unwrap();
        let headers = registry_headers("must-not-cross", None).unwrap();
        client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{source_origin}/start")).unwrap(),
                headers,
                "redirect fixture",
            )
            .await
            .unwrap();
        assert!(source_requests.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer must-not-cross"));
        assert!(!target_requests.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("authorization:"));
        source_server.abort();
        target_server.abort();
    }

    #[tokio::test]
    async fn manual_redirects_reject_loop_missing_location_downgrade_and_hop_eleven() {
        let (origin, _requests, server) = start_scripted_server(|_| {
            vec![TestResponse::new("302 Found", Vec::new()).header("Location", "/start")]
        })
        .await;
        let mut client = OciClient::new_for_http_fixture().unwrap();
        let error = client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{origin}/start")).unwrap(),
                HeaderMap::new(),
                "loop fixture",
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("redirect loop"));
        server.abort();

        let (origin, _requests, server) =
            start_scripted_server(|_| vec![TestResponse::new("302 Found", Vec::new())]).await;
        let error = client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{origin}/start")).unwrap(),
                HeaderMap::new(),
                "missing fixture",
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("missing Location"));
        server.abort();

        let (origin, _requests, server) = start_scripted_server(|_| {
            vec![TestResponse::new("302 Found", Vec::new()).header("Location", "https://[")]
        })
        .await;
        let error = client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{origin}/start")).unwrap(),
                HeaderMap::new(),
                "malformed fixture",
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid Location"));
        server.abort();

        const LOCATION_SECRET: &str = "sentinel-location-secret";
        let (origin, _requests, server) = start_scripted_server(|_| {
            vec![TestResponse::new("302 Found", Vec::new()).header(
                "Location",
                format!("https://user:{LOCATION_SECRET}@127.0.0.1/next?{LOCATION_SECRET}"),
            )]
        })
        .await;
        let error = client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{origin}/start")).unwrap(),
                HeaderMap::new(),
                "userinfo fixture",
            )
            .await
            .unwrap_err();
        let rendered = format!("{error:#}");
        assert!(rendered.contains("user information"));
        assert!(!rendered.contains(LOCATION_SECRET));
        server.abort();

        let mut production = OciClient::new(&BTreeMap::new()).unwrap();
        let error = production
            .get_following_redirects(
                reqwest::Url::parse("http://127.0.0.1:9/secret?sentinel-query").unwrap(),
                HeaderMap::new(),
                "downgrade fixture",
            )
            .await
            .unwrap_err();
        let rendered = format!("{error:#}");
        assert!(rendered.contains("must use HTTPS"));
        assert!(!rendered.contains("sentinel-query"));

        let (origin, requests, server) = start_scripted_server(|_| {
            (0..=MAX_REDIRECTS)
                .map(|index| {
                    TestResponse::new("302 Found", Vec::new())
                        .header("Location", format!("/hop/{}", index + 1))
                })
                .collect()
        })
        .await;
        let error = client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{origin}/hop/0")).unwrap(),
                HeaderMap::new(),
                "hop fixture",
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("maximum of 10 redirects"));
        assert_eq!(requests.lock().unwrap().len(), MAX_REDIRECTS + 1);
        server.abort();
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

    #[tokio::test]
    async fn private_tls_registry_requires_exact_root_and_valid_hostname() {
        const SECRET_PATH: &str = "sentinel-private-registry-path";
        let ca = test_ca();
        let server_config = tls_server_config(&ca, vec!["127.0.0.1".to_string()]);
        let blob = feature_tar(None);
        let manifest = feature_manifest(&blob);
        let (origin, requests, server) = start_tls_scripted_server(server_config, |_| {
            vec![
                TestResponse::new("200 OK", Vec::new()),
                TestResponse::new("200 OK", manifest).header("Content-Type", "application/json"),
                TestResponse::new("200 OK", blob),
            ]
        })
        .await;
        let reference = format!("{}/owner/{SECRET_PATH}:1", origin_authority(&origin));

        let missing_error = OciClient::new(&BTreeMap::new())
            .unwrap()
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert_chain_contains(
            &missing_error,
            &["unknownissuer", "unknown issuer", "badsignature"],
        );
        assert!(!error_chain_text(&missing_error).contains(SECRET_PATH));

        let wrong_ca = test_ca();
        let wrong_error = configured_client([(origin_authority(&origin), ca_bundle(&wrong_ca))])
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert_chain_contains(
            &wrong_error,
            &["unknownissuer", "unknown issuer", "badsignature"],
        );
        assert!(!error_chain_text(&wrong_error).contains(SECRET_PATH));

        let feature = configured_client([(origin_authority(&origin), ca_bundle(&ca))])
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(feature.install_sh, b"#!/bin/sh\nprintf installed\n");
        assert_eq!(requests.lock().unwrap().len(), 3);
        server.abort();

        let hostname_ca = test_ca();
        let bad_hostname_config =
            tls_server_config(&hostname_ca, vec!["wrong.example.test".to_string()]);
        let (origin, requests, server) = start_tls_scripted_server(bad_hostname_config, |_| {
            vec![TestResponse::new("200 OK", Vec::new())]
        })
        .await;
        let reference = format!("{}/owner/{SECRET_PATH}:1", origin_authority(&origin));
        let error = configured_client([(origin_authority(&origin), ca_bundle(&hostname_ca))])
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert_chain_contains(&error, &["notvalidforname", "not valid for name"]);
        assert!(!error_chain_text(&error).contains(SECRET_PATH));
        assert!(requests.lock().unwrap().is_empty());
        server.abort();

        let expired_ca = test_ca();
        let mut expired_params = CertificateParams::new(vec!["127.0.0.1".to_string()]).unwrap();
        expired_params.not_before = rcgen::date_time_ymd(2000, 1, 1);
        expired_params.not_after = rcgen::date_time_ymd(2001, 1, 1);
        let expired_config = tls_server_config_with_params(&expired_ca, expired_params);
        let (origin, requests, server) = start_tls_scripted_server(expired_config, |_| {
            vec![TestResponse::new("200 OK", Vec::new())]
        })
        .await;
        let reference = format!("{}/owner/{SECRET_PATH}:1", origin_authority(&origin));
        let error = configured_client([(origin_authority(&origin), ca_bundle(&expired_ca))])
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert_chain_contains(&error, &["expired"]);
        assert!(!error_chain_text(&error).contains(SECRET_PATH));
        assert!(requests.lock().unwrap().is_empty());
        server.abort();

        let future_ca = test_ca();
        let mut future_params = CertificateParams::new(vec!["127.0.0.1".to_string()]).unwrap();
        future_params.not_before = rcgen::date_time_ymd(2099, 1, 1);
        future_params.not_after = rcgen::date_time_ymd(2100, 1, 1);
        let future_config = tls_server_config_with_params(&future_ca, future_params);
        let (origin, requests, server) = start_tls_scripted_server(future_config, |_| {
            vec![TestResponse::new("200 OK", Vec::new())]
        })
        .await;
        let reference = format!("{}/owner/{SECRET_PATH}:1", origin_authority(&origin));
        let error = configured_client([(origin_authority(&origin), ca_bundle(&future_ca))])
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap_err();
        assert_chain_contains(&error, &["notvalidyet", "not valid yet"]);
        assert!(!error_chain_text(&error).contains(SECRET_PATH));
        assert!(requests.lock().unwrap().is_empty());
        server.abort();
    }

    #[tokio::test]
    async fn custom_bundle_accepts_a_signer_after_other_and_repeated_roots() {
        let unrelated = test_ca();
        let signer = test_ca();
        let blob = feature_tar(None);
        let manifest = feature_manifest(&blob);
        let (origin, requests, server) = start_tls_scripted_server(
            tls_server_config(&signer, vec!["127.0.0.1".to_string()]),
            |_| {
                vec![
                    TestResponse::new("200 OK", Vec::new()),
                    TestResponse::new("200 OK", manifest)
                        .header("Content-Type", "application/json"),
                    TestResponse::new("200 OK", blob),
                ]
            },
        )
        .await;
        let reference = format!("{}/owner/feature:1", origin_authority(&origin));
        let bundle = ca_bundle_many([&unrelated, &signer, &signer]);
        configured_client([(origin_authority(&origin), bundle)])
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(requests.lock().unwrap().len(), 3);
        server.abort();
    }

    #[tokio::test]
    async fn baseline_roots_remain_available_in_public_and_custom_clients() {
        let baseline = test_ca();
        let unrelated_custom = test_ca();
        let (public_origin, public_requests, public_server) = start_tls_scripted_server(
            tls_server_config(&baseline, vec!["127.0.0.1".to_string()]),
            |_| vec![TestResponse::new("200 OK", Vec::new())],
        )
        .await;
        let (custom_origin, custom_requests, custom_server) = start_tls_scripted_server(
            tls_server_config(&baseline, vec!["127.0.0.1".to_string()]),
            |_| vec![TestResponse::new("200 OK", Vec::new())],
        )
        .await;
        let configured = BTreeMap::from([(
            RegistryAuthority::parse(&origin_authority(&custom_origin)).unwrap(),
            ca_bundle(&unrelated_custom),
        )]);
        let mut client = OciClient::new_with_baseline_roots(
            &configured,
            vec![reqwest::Certificate::from_der(baseline.der()).unwrap()],
        )
        .unwrap();

        for origin in [&public_origin, &custom_origin] {
            client
                .get_following_redirects(
                    reqwest::Url::parse(&format!("{origin}/v2/")).unwrap(),
                    HeaderMap::new(),
                    "baseline root fixture",
                )
                .await
                .unwrap();
        }
        assert_eq!(public_requests.lock().unwrap().len(), 1);
        assert_eq!(custom_requests.lock().unwrap().len(), 1);
        public_server.abort();
        custom_server.abort();
    }

    #[tokio::test]
    async fn custom_root_does_not_bleed_to_an_unconfigured_authority() {
        let ca = test_ca();
        let (first_origin, first_requests, first_server) = start_tls_scripted_server(
            tls_server_config(&ca, vec!["127.0.0.1".to_string()]),
            |_| vec![TestResponse::new("200 OK", b"first".as_slice())],
        )
        .await;
        let (second_origin, second_requests, second_server) = start_tls_scripted_server(
            tls_server_config(&ca, vec!["127.0.0.1".to_string()]),
            |_| vec![TestResponse::new("200 OK", b"second".as_slice())],
        )
        .await;
        let mut client = configured_client([(origin_authority(&first_origin), ca_bundle(&ca))]);
        assert!(client.registry_clients.is_empty());
        client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{first_origin}/v2/")).unwrap(),
                HeaderMap::new(),
                "configured authority fixture",
            )
            .await
            .unwrap();
        assert_eq!(client.registry_clients.len(), 1);
        let error = client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{second_origin}/v2/")).unwrap(),
                HeaderMap::new(),
                "unconfigured authority fixture",
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("unconfigured authority fixture failed"));
        assert_eq!(client.registry_clients.len(), 1);
        assert_eq!(first_requests.lock().unwrap().len(), 1);
        assert!(second_requests.lock().unwrap().is_empty());
        first_server.abort();
        second_server.abort();
    }

    #[tokio::test]
    async fn tls_cross_authority_redirect_reselects_trust_and_strips_authorization() {
        let ca = test_ca();
        let (target_origin, target_requests, target_server) = start_tls_scripted_server(
            tls_server_config(&ca, vec!["127.0.0.1".to_string()]),
            |_| vec![TestResponse::new("200 OK", b"target".as_slice())],
        )
        .await;
        let (source_origin, source_requests, source_server) = start_tls_scripted_server(
            tls_server_config(&ca, vec!["127.0.0.1".to_string()]),
            |_| {
                vec![TestResponse::new("302 Found", Vec::new())
                    .header("Location", format!("{target_origin}/blob"))]
            },
        )
        .await;
        let mut client = configured_client([
            (origin_authority(&source_origin), ca_bundle(&ca)),
            (origin_authority(&target_origin), ca_bundle(&ca)),
        ]);
        client
            .get_following_redirects(
                reqwest::Url::parse(&format!("{source_origin}/manifest")).unwrap(),
                registry_headers("redirect-secret", None).unwrap(),
                "TLS redirect fixture",
            )
            .await
            .unwrap();
        assert!(source_requests.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer redirect-secret"));
        assert!(!target_requests.lock().unwrap()[0]
            .to_ascii_lowercase()
            .contains("authorization:"));
        source_server.abort();
        target_server.abort();
    }

    #[tokio::test]
    async fn split_bearer_realm_requires_its_own_exact_root() {
        const TOKEN: &str = "split-realm-token";
        let registry_ca = test_ca();
        let auth_ca = test_ca();
        let (auth_origin, auth_requests, auth_server) = start_tls_scripted_server(
            tls_server_config(&auth_ca, vec!["127.0.0.1".to_string()]),
            |_| {
                vec![TestResponse::new(
                    "200 OK",
                    format!(r#"{{"token":"{TOKEN}"}}"#),
                )]
            },
        )
        .await;
        let blob = feature_tar(None);
        let manifest = feature_manifest(&blob);
        let (registry_origin, registry_requests, registry_server) = start_tls_scripted_server(
            tls_server_config(&registry_ca, vec!["127.0.0.1".to_string()]),
            |_| {
                let challenge = || {
                    TestResponse::new("401 Unauthorized", Vec::new()).header(
                        "WWW-Authenticate",
                        format!(
                            "Bearer realm=\"{auth_origin}/token?existing=1\",service=\"fixture\""
                        ),
                    )
                };
                vec![
                    challenge(),
                    challenge(),
                    TestResponse::new("200 OK", manifest)
                        .header("Content-Type", "application/json"),
                    TestResponse::new("200 OK", blob),
                ]
            },
        )
        .await;
        let reference = format!("{}/owner/feature:1", origin_authority(&registry_origin));
        let missing_realm_error =
            configured_client([(origin_authority(&registry_origin), ca_bundle(&registry_ca))])
                .download_feature(&reference, &serde_json::json!({}))
                .await
                .unwrap_err();
        assert!(format!("{missing_realm_error:#}").contains("registry token request failed"));

        configured_client([
            (origin_authority(&registry_origin), ca_bundle(&registry_ca)),
            (origin_authority(&auth_origin), ca_bundle(&auth_ca)),
        ])
        .download_feature(&reference, &serde_json::json!({}))
        .await
        .unwrap();

        let auth_request = &auth_requests.lock().unwrap()[0];
        assert!(request_path(auth_request).contains("existing=1"));
        assert!(request_path(auth_request).contains("service=fixture"));
        assert!(request_path(auth_request).contains("scope=repository%3Aowner%2Ffeature%3Apull"));
        assert!(!auth_request.to_ascii_lowercase().contains("authorization:"));
        let registry_requests = registry_requests.lock().unwrap();
        assert!(registry_requests[2]
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {TOKEN}")));
        assert!(registry_requests[3]
            .to_ascii_lowercase()
            .contains(&format!("authorization: bearer {TOKEN}")));
        drop(registry_requests);
        registry_server.abort();
        auth_server.abort();
    }

    #[tokio::test]
    async fn token_endpoint_redirect_preserves_queries_and_sends_no_authorization() {
        const TOKEN: &str = "redirected-token";
        let (target_origin, target_requests, target_server) = start_scripted_server(|_| {
            vec![TestResponse::new(
                "200 OK",
                format!(r#"{{"token":"{TOKEN}"}}"#),
            )]
        })
        .await;
        let redirect_location = format!(
            "{target_origin}/final?existing=1&service=fixture&scope=repository%3Aowner%2Ffeature%3Apull"
        );
        let (token_origin, token_requests, token_server) = start_scripted_server(|_| {
            vec![TestResponse::new("302 Found", Vec::new()).header("Location", redirect_location)]
        })
        .await;
        let blob = feature_tar(None);
        let manifest = feature_manifest(&blob);
        let (registry_origin, registry_requests, registry_server) = start_scripted_server(|_| {
            vec![
                TestResponse::new("401 Unauthorized", Vec::new()).header(
                    "WWW-Authenticate",
                    format!("Bearer realm=\"{token_origin}/token?existing=1\",service=\"fixture\""),
                ),
                TestResponse::new("200 OK", manifest).header("Content-Type", "application/json"),
                TestResponse::new("200 OK", blob),
            ]
        })
        .await;
        let reference = format!("{}/owner/feature:1", origin_authority(&registry_origin));
        OciClient::new_for_http_fixture()
            .unwrap()
            .download_feature(&reference, &serde_json::json!({}))
            .await
            .unwrap();

        let token_requests = token_requests.lock().unwrap();
        assert_eq!(token_requests.len(), 1);
        let initial_path = request_path(&token_requests[0]);
        assert!(initial_path.contains("existing=1"));
        assert!(initial_path.contains("service=fixture"));
        assert!(initial_path.contains("scope=repository%3Aowner%2Ffeature%3Apull"));
        assert!(!token_requests[0]
            .to_ascii_lowercase()
            .contains("authorization:"));
        drop(token_requests);

        let target_requests = target_requests.lock().unwrap();
        assert_eq!(target_requests.len(), 1);
        let redirected_path = request_path(&target_requests[0]);
        assert!(redirected_path.contains("existing=1"));
        assert!(redirected_path.contains("service=fixture"));
        assert!(redirected_path.contains("scope=repository%3Aowner%2Ffeature%3Apull"));
        assert!(!target_requests[0]
            .to_ascii_lowercase()
            .contains("authorization:"));
        drop(target_requests);
        assert_eq!(registry_requests.lock().unwrap().len(), 3);
        registry_server.abort();
        token_server.abort();
        target_server.abort();
    }

    #[tokio::test]
    async fn token_cache_is_scoped_by_authority_and_repository_scope() {
        let (origin, requests, server) = start_scripted_server(|origin| {
            vec![
                TestResponse::new("401 Unauthorized", Vec::new()).header(
                    "WWW-Authenticate",
                    format!("Bearer realm=\"{origin}/token-a\",service=\"fixture\""),
                ),
                TestResponse::new("200 OK", br#"{"token":"token-a"}"#.as_slice()),
                TestResponse::new("401 Unauthorized", Vec::new()).header(
                    "WWW-Authenticate",
                    format!("Bearer realm=\"{origin}/token-b\",service=\"fixture\""),
                ),
                TestResponse::new("200 OK", br#"{"token":"token-b"}"#.as_slice()),
            ]
        })
        .await;
        let authority = RegistryAuthority::parse(&origin_authority(&origin)).unwrap();
        let mut client = OciClient::new_for_http_fixture().unwrap();
        assert_eq!(
            client
                .authenticate(&authority, "repository:owner/a:pull")
                .await
                .unwrap(),
            "token-a"
        );
        assert_eq!(
            client
                .authenticate(&authority, "repository:owner/a:pull")
                .await
                .unwrap(),
            "token-a"
        );
        assert_eq!(
            client
                .authenticate(&authority, "repository:owner/b:pull")
                .await
                .unwrap(),
            "token-b"
        );
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests
                .iter()
                .filter(|request| request_path(request) == "/v2/")
                .count(),
            2
        );
        drop(requests);
        server.abort();
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
    fn extract_feature_accepts_archive_root_directory_entry() {
        let mut buf = Vec::new();
        let mut builder = tar::Builder::new(&mut buf);
        raw_tar_entry(&mut builder, b"./", tar::EntryType::Directory);
        tar_entry(&mut builder, "install.sh", b"#!/bin/sh\n", 0o755);
        builder.finish().unwrap();
        drop(builder);

        let (install_sh, feature_json, extra_files) = extract_feature(&buf).unwrap();
        assert_eq!(install_sh, b"#!/bin/sh\n");
        assert!(feature_json.is_none());
        assert!(extra_files.is_empty());
    }

    #[test]
    fn extract_feature_rejects_non_directory_archive_root_entry() {
        let mut buf = Vec::new();
        let mut builder = tar::Builder::new(&mut buf);
        tar_entry(&mut builder, "install.sh", b"#!/bin/sh\n", 0o755);
        raw_tar_entry(&mut builder, b"./", tar::EntryType::Regular);
        builder.finish().unwrap();
        drop(builder);

        let error = extract_feature(&buf).unwrap_err();
        assert!(error.to_string().contains("empty path"));
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
            prop_assert_eq!(parsed.registry.to_string(), registry);
            prop_assert_eq!(parsed.repository, repository);
            prop_assert_eq!(parsed.tag, tag);
        }
    }
}
