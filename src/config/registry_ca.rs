use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    fs::File,
    io::{Cursor, Read as _},
    net::{Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context as _};
use serde::de::{Error as _, MapAccess, Visitor};

const MAX_CA_BUNDLE_BYTES: u64 = 1024 * 1024;
const CERTIFICATE_BEGIN: &[u8] = b"-----BEGIN CERTIFICATE-----";
const CERTIFICATE_END: &[u8] = b"-----END CERTIFICATE-----";

/// An exact OCI network authority, canonicalized for trust selection.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub(crate) struct RegistryAuthority(String);

impl RegistryAuthority {
    pub(crate) fn parse(input: &str) -> anyhow::Result<Self> {
        if input.is_empty() {
            bail!("registry authority is empty");
        }
        if !input.is_ascii() {
            bail!("registry authority must be ASCII");
        }
        if input.bytes().any(|byte| byte.is_ascii_whitespace()) {
            bail!("registry authority must not contain whitespace");
        }
        if input.contains("://")
            || input.contains('@')
            || input.contains('/')
            || input.contains('?')
            || input.contains('#')
        {
            bail!("registry authority must contain only a host and optional numeric port");
        }

        let (host, port, ipv6) = if let Some(rest) = input.strip_prefix('[') {
            let close = rest
                .find(']')
                .ok_or_else(|| anyhow::anyhow!("registry authority has invalid IPv6 brackets"))?;
            let host = &rest[..close];
            let suffix = &rest[close + 1..];
            let port = parse_port_suffix(suffix)?;
            let address: Ipv6Addr = host
                .parse()
                .context("registry authority has an invalid IPv6 address")?;
            (address.to_string(), port, true)
        } else {
            if input.contains('[') || input.contains(']') {
                bail!("registry authority has invalid IPv6 brackets");
            }
            if input.matches(':').count() > 1 {
                bail!("registry authority must bracket an IPv6 literal");
            }
            let (host, port) = match input.rsplit_once(':') {
                Some((host, port)) => (host, Some(parse_port(port)?)),
                None => (input, None),
            };
            if host.is_empty() {
                bail!("registry authority has an empty host");
            }
            if host.ends_with('.') {
                bail!("registry authority must not have a trailing dot");
            }
            let host = match host.parse::<Ipv4Addr>() {
                Ok(address) => address.to_string(),
                Err(_) => canonical_dns_name(host)?,
            };
            (host, port, false)
        };

        let port = port.filter(|port| *port != 443);
        let canonical = match (ipv6, port) {
            (true, Some(port)) => format!("[{host}]:{port}"),
            (true, None) => format!("[{host}]"),
            (false, Some(port)) => format!("{host}:{port}"),
            (false, None) => host,
        };
        Ok(Self(canonical))
    }

    pub(crate) fn from_url(url: &reqwest::Url) -> anyhow::Result<Self> {
        if !url.username().is_empty() || url.password().is_some() {
            bail!("request URL must not contain user information");
        }
        let host = url
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("request URL has no host"))?;
        let host = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        let authority = match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host,
        };
        Self::parse(&authority)
    }
}

impl fmt::Display for RegistryAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

fn parse_port_suffix(suffix: &str) -> anyhow::Result<Option<u16>> {
    if suffix.is_empty() {
        return Ok(None);
    }
    let port = suffix
        .strip_prefix(':')
        .ok_or_else(|| anyhow::anyhow!("registry authority has content after its IPv6 literal"))?;
    Ok(Some(parse_port(port)?))
}

fn parse_port(port: &str) -> anyhow::Result<u16> {
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("registry authority has an invalid port");
    }
    let port: u16 = port
        .parse()
        .context("registry authority has an invalid port")?;
    if port == 0 {
        bail!("registry authority must not use port 0");
    }
    Ok(port)
}

fn canonical_dns_name(host: &str) -> anyhow::Result<String> {
    if host.len() > 253 {
        bail!("registry authority has a DNS name longer than 253 bytes");
    }
    if host
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        bail!("registry authority has an invalid IPv4 address");
    }
    for label in host.split('.') {
        if label.is_empty()
            || label.len() > 63
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || label.starts_with('-')
            || label.ends_with('-')
        {
            bail!("registry authority has an invalid DNS name");
        }
    }
    Ok(host.to_ascii_lowercase())
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RegistryCaSource {
    path: PathBuf,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct RawRegistryCas(pub(crate) BTreeMap<RegistryAuthority, RegistryCaSource>);

impl RawRegistryCas {
    pub(crate) fn anchor_paths(&mut self, source: &Path) -> anyhow::Result<()> {
        let parent = source.parent().with_context(|| {
            format!(
                "`{}` declares customizations.dcc.registryCAs but has no parent directory",
                source.display()
            )
        })?;
        for entry in self.0.values_mut() {
            if entry.path.is_relative() {
                entry.path = std::path::absolute(parent.join(&entry.path)).with_context(|| {
                    format!(
                        "failed to resolve a customizations.dcc.registryCAs path declared in `{}`",
                        source.display()
                    )
                })?;
            }
        }
        Ok(())
    }

    pub(crate) fn merge(mut self, child: Self) -> Self {
        self.0.extend(child.0);
        self
    }

    pub(crate) fn validate(self) -> anyhow::Result<BTreeMap<RegistryAuthority, RegistryCaBundle>> {
        self.0
            .into_iter()
            .map(|(authority, source)| {
                let bundle = load_bundle(&authority, &source)?;
                Ok((authority, bundle))
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn path_for(&self, authority: &RegistryAuthority) -> Option<&Path> {
        self.0.get(authority).map(|source| source.path.as_path())
    }
}

impl<'de> serde::Deserialize<'de> for RawRegistryCas {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RegistryCasVisitor;

        impl<'de> Visitor<'de> for RegistryCasVisitor {
            type Value = RawRegistryCas;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an object mapping registry authorities to PEM bundle paths")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut entries = BTreeMap::new();
                let mut spellings = BTreeMap::<RegistryAuthority, String>::new();
                while let Some((raw_authority, path)) = map.next_entry::<String, String>()? {
                    let authority =
                        RegistryAuthority::parse(&raw_authority).map_err(A::Error::custom)?;
                    if let Some(previous) = spellings.get(&authority) {
                        return Err(A::Error::custom(format!(
                            "duplicate registry authority `{raw_authority}` canonicalizes to `{authority}`, already declared as `{previous}`"
                        )));
                    }
                    spellings.insert(authority.clone(), raw_authority);
                    entries.insert(
                        authority,
                        RegistryCaSource {
                            path: PathBuf::from(path),
                        },
                    );
                }
                Ok(RawRegistryCas(entries))
            }
        }

        deserializer.deserialize_map(RegistryCasVisitor)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RegistryCaBundle {
    pub(crate) certificates: Vec<reqwest::Certificate>,
}

fn load_bundle(
    authority: &RegistryAuthority,
    source: &RegistryCaSource,
) -> anyhow::Result<RegistryCaBundle> {
    let mut file = File::open(&source.path).with_context(|| {
        format!(
            "failed to read custom CA bundle for {authority} from `{}`",
            source.path.display()
        )
    })?;
    let metadata = file.metadata().with_context(|| {
        format!(
            "failed to inspect custom CA bundle for {authority} at `{}`",
            source.path.display()
        )
    })?;
    if !metadata.is_file() {
        bail!("custom CA bundle for {authority} must be a regular file");
    }
    if metadata.len() > MAX_CA_BUNDLE_BYTES {
        bail!("custom CA bundle for {authority} exceeds the 1 MiB limit");
    }

    let mut pem = Vec::new();
    file.by_ref()
        .take(MAX_CA_BUNDLE_BYTES + 1)
        .read_to_end(&mut pem)
        .with_context(|| {
            format!(
                "failed to read custom CA bundle for {authority} from `{}`",
                source.path.display()
            )
        })?;
    if pem.len() as u64 > MAX_CA_BUNDLE_BYTES {
        bail!("custom CA bundle for {authority} exceeds the 1 MiB limit");
    }
    validate_strict_pem_framing(&pem)
        .with_context(|| format!("invalid custom CA bundle for {authority}"))?;

    let items: Vec<_> = rustls_pemfile::read_all(&mut Cursor::new(&pem))
        .collect::<Result<_, _>>()
        .with_context(|| format!("invalid custom CA bundle for {authority}"))?;
    let mut seen = HashSet::new();
    let mut certificates = Vec::new();
    for item in items {
        let rustls_pemfile::Item::X509Certificate(der) = item else {
            bail!("invalid custom CA bundle for {authority}: unsupported PEM object");
        };
        if seen.insert(der.as_ref().to_vec()) {
            certificates.push(
                reqwest::Certificate::from_der(der.as_ref())
                    .with_context(|| format!("invalid custom CA bundle for {authority}"))?,
            );
        }
    }
    if certificates.is_empty() {
        bail!("invalid custom CA bundle for {authority}: no certificates found");
    }

    let mut builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
    for certificate in &certificates {
        builder = builder.add_root_certificate(certificate.clone());
    }
    builder
        .build()
        .with_context(|| format!("invalid custom CA bundle for {authority}"))?;

    Ok(RegistryCaBundle { certificates })
}

fn validate_strict_pem_framing(pem: &[u8]) -> anyhow::Result<()> {
    let mut remaining = pem;
    let mut count = 0usize;
    loop {
        remaining = trim_ascii_whitespace_start(remaining);
        if remaining.is_empty() {
            break;
        }
        if !remaining.starts_with(CERTIFICATE_BEGIN) {
            bail!("bundle contains data other than CERTIFICATE PEM blocks");
        }
        let end = find_bytes(remaining, CERTIFICATE_END)
            .ok_or_else(|| anyhow::anyhow!("certificate PEM block is truncated"))?;
        remaining = &remaining[end + CERTIFICATE_END.len()..];
        count += 1;
    }
    if count == 0 {
        bail!("bundle contains no certificate PEM blocks");
    }
    Ok(())
}

fn trim_ascii_whitespace_start(mut bytes: &[u8]) -> &[u8] {
    while bytes.first().is_some_and(|byte| byte.is_ascii_whitespace()) {
        bytes = &bytes[1..];
    }
    bytes
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn certificate_pem() -> String {
        rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
            .unwrap()
            .cert
            .pem()
    }

    fn parse_map(json: &str) -> anyhow::Result<RawRegistryCas> {
        json5::from_str(json).map_err(Into::into)
    }

    #[test]
    fn authority_canonicalizes_dns_ip_and_default_port() {
        assert_eq!(
            RegistryAuthority::parse("EXAMPLE.test").unwrap(),
            RegistryAuthority("example.test".to_string())
        );
        assert_eq!(
            RegistryAuthority::parse("example.test:443").unwrap(),
            RegistryAuthority("example.test".to_string())
        );
        assert_eq!(
            RegistryAuthority::parse("example.test:0443").unwrap(),
            RegistryAuthority("example.test".to_string())
        );
        assert_eq!(
            RegistryAuthority::parse("example.test:5443").unwrap(),
            RegistryAuthority("example.test:5443".to_string())
        );
        assert_eq!(
            RegistryAuthority::parse("127.0.0.1:5443").unwrap(),
            RegistryAuthority("127.0.0.1:5443".to_string())
        );
        assert_eq!(
            RegistryAuthority::parse("[2001:0DB8::1]:443").unwrap(),
            RegistryAuthority("[2001:db8::1]".to_string())
        );
    }

    #[test]
    fn authority_rejects_invalid_spellings() {
        for authority in [
            "",
            "https://example.test",
            "user@example.test",
            "example.test/path",
            "example.test?x",
            "example.test#x",
            "example.test ",
            "example.test.",
            "example.test:0",
            "example.test:no",
            "::1",
            "[::1",
            "127.1",
            "bad_name",
            "-bad.test",
            "bad-.test",
            "bad..test",
            "café.test",
        ] {
            assert!(
                RegistryAuthority::parse(authority).is_err(),
                "unexpectedly accepted {authority:?}"
            );
        }
    }

    #[test]
    fn invalid_authority_errors_do_not_echo_url_secrets() {
        for authority in [
            "sentinel-user:sentinel-password@example.test",
            "example.test/path/sentinel-path",
            "example.test?token=sentinel-query",
            "example.test#sentinel-fragment",
            "https://sentinel-scheme.example.test",
        ] {
            let message = RegistryAuthority::parse(authority).unwrap_err().to_string();
            assert!(!message.contains("sentinel"), "{message}");
            assert!(!message.contains(authority), "{message}");
        }
    }

    #[test]
    fn url_authority_rejects_user_information() {
        let url = reqwest::Url::parse("https://user:secret@example.test/v2/").unwrap();
        assert!(RegistryAuthority::from_url(&url).is_err());
    }

    #[test]
    fn url_authority_preserves_ipv6_brackets() {
        let url = reqwest::Url::parse("https://[2001:db8::1]:5443/v2/").unwrap();
        assert_eq!(
            RegistryAuthority::from_url(&url).unwrap(),
            RegistryAuthority("[2001:db8::1]:5443".to_string())
        );
    }

    #[test]
    fn map_rejects_exact_and_canonical_duplicates() {
        for json in [
            r#"{"example.test":"a.pem","example.test":"b.pem"}"#,
            r#"{"EXAMPLE.test":"a.pem","example.test:443":"b.pem"}"#,
            r#"{"example.test:0443":"a.pem","example.test":"b.pem"}"#,
        ] {
            let error = parse_map(json).unwrap_err();
            assert!(error.to_string().contains("duplicate registry authority"));
        }
    }

    #[test]
    fn map_requires_string_paths() {
        assert!(parse_map(r#"{"example.test":42}"#).is_err());
        assert!(parse_map(r#"{"example.test":null}"#).is_err());
        assert!(parse_map(r#"{"example.test":["ca.pem"]}"#).is_err());
    }

    #[test]
    fn valid_bundle_loads_multiple_roots_and_deduplicates_repeats() {
        let temp = tempfile::tempdir().unwrap();
        let first = certificate_pem();
        let second = certificate_pem();
        let path = temp.path().join("bundle.pem");
        std::fs::write(&path, format!("{first}\n{second}\n{first}")).unwrap();
        let source = RegistryCaSource { path };
        let bundle =
            load_bundle(&RegistryAuthority::parse("registry.test").unwrap(), &source).unwrap();
        assert_eq!(bundle.certificates.len(), 2);
    }

    #[test]
    fn bundle_rejects_file_and_pem_failures_without_echoing_contents() {
        let temp = tempfile::tempdir().unwrap();
        let authority = RegistryAuthority::parse("registry.test").unwrap();
        let cases: &[(&str, &[u8])] = &[
            ("empty.pem", b""),
            ("junk.pem", b"not-a-secret-sentinel"),
            (
                "key.pem",
                b"-----BEGIN PRIVATE KEY-----\naA==\n-----END PRIVATE KEY-----\n",
            ),
            (
                "truncated.pem",
                b"-----BEGIN CERTIFICATE-----\naA==\n",
            ),
            (
                "invalid-base64.pem",
                b"-----BEGIN CERTIFICATE-----\n***\n-----END CERTIFICATE-----\n",
            ),
            (
                "invalid-der.pem",
                b"-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\n",
            ),
            (
                "trailing.pem",
                b"-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\nsecret-tail-sentinel",
            ),
        ];
        for (name, contents) in cases {
            let path = temp.path().join(name);
            std::fs::write(&path, contents).unwrap();
            let error = load_bundle(&authority, &RegistryCaSource { path }).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("registry.test"), "{name}: {message}");
            assert!(!message.contains("secret"), "{name}: {message}");
            assert!(!message.contains("AQID"), "{name}: {message}");
        }

        let missing = temp.path().join("missing-ca.pem");
        let message = format!(
            "{:#}",
            load_bundle(
                &authority,
                &RegistryCaSource {
                    path: missing.clone()
                }
            )
            .unwrap_err()
        );
        assert!(message.contains(&missing.display().to_string()));

        let error = load_bundle(
            &authority,
            &RegistryCaSource {
                path: temp.path().to_owned(),
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("regular file"));
    }

    #[test]
    fn bundle_rejects_oversized_files_before_parsing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("large.pem");
        std::fs::write(&path, vec![b' '; MAX_CA_BUNDLE_BYTES as usize + 1]).unwrap();
        let error = load_bundle(
            &RegistryAuthority::parse("registry.test").unwrap(),
            &RegistryCaSource { path },
        )
        .unwrap_err();
        assert!(error.to_string().contains("1 MiB"));
    }

    #[test]
    fn bundle_accepts_exactly_one_mib() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("exact-limit.pem");
        let mut contents = certificate_pem().into_bytes();
        contents.resize(MAX_CA_BUNDLE_BYTES as usize, b' ');
        assert_eq!(contents.len() as u64, MAX_CA_BUNDLE_BYTES);
        std::fs::write(&path, contents).unwrap();

        let bundle = load_bundle(
            &RegistryAuthority::parse("registry.test").unwrap(),
            &RegistryCaSource { path },
        )
        .unwrap();
        assert_eq!(bundle.certificates.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn bundle_rejects_unreadable_files() {
        use std::os::unix::fs::PermissionsExt as _;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("unreadable.pem");
        std::fs::write(&path, certificate_pem()).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o0)).unwrap();
        let error = load_bundle(
            &RegistryAuthority::parse("registry.test").unwrap(),
            &RegistryCaSource { path: path.clone() },
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains(&path.display().to_string()));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn strict_framing_rejects_junk_and_other_objects() {
        assert!(validate_strict_pem_framing(b"").is_err());
        assert!(validate_strict_pem_framing(b"junk").is_err());
        assert!(validate_strict_pem_framing(
            b"-----BEGIN PRIVATE KEY-----\naA==\n-----END PRIVATE KEY-----\n"
        )
        .is_err());
        assert!(validate_strict_pem_framing(
            b"-----BEGIN CERTIFICATE-----\naA==\n-----END CERTIFICATE-----\njunk"
        )
        .is_err());
        assert!(validate_strict_pem_framing(
            b"-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\n-----BEGIN PRIVATE KEY-----\naA==\n-----END PRIVATE KEY-----\n"
        )
        .is_err());
    }
}
