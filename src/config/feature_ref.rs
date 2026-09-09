use std::{fmt, str::FromStr};

use anyhow::{bail, Context as _};
use serde::de::Error as _;

use super::registry_ca::RegistryAuthority;

pub(crate) const BUILTIN_DEFAULT_FEATURE_REPOSITORY: &str = "ghcr.io/dc-powertools/features";
const IMPLICIT_TAG: &str = "latest";

/// A validated OCI prefix used to qualify project-declared short Feature names.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct DefaultFeatureRepository {
    authority: RegistryAuthority,
    path: String,
}

impl DefaultFeatureRepository {
    fn qualify(&self, name: &str, tag: &str) -> String {
        format!("{}/{}/{}:{}", self.authority, self.path, name, tag)
    }
}

impl FromStr for DefaultFeatureRepository {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let (authority, path) = input.split_once('/').ok_or_else(|| {
            anyhow::anyhow!(
                "default Feature repository must contain a registry authority and repository path"
            )
        })?;
        let authority = RegistryAuthority::parse(authority)
            .context("default Feature repository has an invalid registry authority")?;
        validate_repository_path(path).context("default Feature repository has an invalid path")?;
        Ok(Self {
            authority,
            path: path.to_owned(),
        })
    }
}

impl fmt::Display for DefaultFeatureRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.authority, self.path)
    }
}

impl<'de> serde::Deserialize<'de> for DefaultFeatureRepository {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        input.parse().map_err(D::Error::custom)
    }
}

/// A fully qualified OCI Feature identity with an explicit effective tag.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct OciFeatureReference {
    pub(crate) registry: RegistryAuthority,
    pub(crate) repository: String,
    pub(crate) tag: String,
}

impl OciFeatureReference {
    pub(crate) fn parse(input: &str) -> anyhow::Result<Self> {
        if input.starts_with("./") || input.starts_with("../") {
            bail!("local Feature paths are not OCI references");
        }
        if !input.is_ascii() || input.bytes().any(|byte| byte.is_ascii_whitespace()) {
            bail!("OCI Feature reference must be ASCII and contain no whitespace");
        }
        if input.contains('@') {
            bail!("OCI Feature digest references are not supported");
        }

        let slash = input.find('/').ok_or_else(|| {
            anyhow::anyhow!("OCI Feature reference must include a registry authority")
        })?;
        let registry = RegistryAuthority::parse(&input[..slash])
            .context("OCI Feature reference has an invalid registry authority")?;
        let remainder = &input[slash + 1..];
        let (repository, tag) = match remainder.rsplit_once(':') {
            Some((repository, "")) => {
                let _ = repository;
                bail!("OCI Feature reference has an empty tag")
            }
            Some((repository, tag)) => (repository, tag),
            None => (remainder, IMPLICIT_TAG),
        };
        validate_repository_path(repository)
            .context("OCI Feature reference has an invalid path")?;
        validate_tag(tag).context("OCI Feature reference has an invalid tag")?;
        Ok(Self {
            registry,
            repository: repository.to_owned(),
            tag: tag.to_owned(),
        })
    }

    pub(crate) fn canonical(&self) -> String {
        format!("{}/{}:{}", self.registry, self.repository, self.tag)
    }
}

pub(crate) fn is_local_feature(reference: &str) -> bool {
    reference.starts_with("./") || reference.starts_with("../")
}

/// Resolves a Feature key declared by a project config into its stable identity.
pub(crate) fn normalize_project_feature(
    reference: &str,
    repository_override: Option<&DefaultFeatureRepository>,
) -> anyhow::Result<String> {
    if is_local_feature(reference) {
        return Ok(reference.to_owned());
    }
    if reference.contains('/') {
        return Ok(OciFeatureReference::parse(reference)?.canonical());
    }

    let (name, tag) = parse_short_reference(reference)?;
    if let Some(repository_override) = repository_override {
        return Ok(repository_override.qualify(name, tag));
    }
    let builtin: DefaultFeatureRepository = BUILTIN_DEFAULT_FEATURE_REPOSITORY
        .parse()
        .context("built-in default Feature repository is invalid")?;
    Ok(builtin.qualify(name, tag))
}

/// Normalizes a Feature-authored dependency without applying a project default.
pub(crate) fn normalize_feature_dependency(reference: &str) -> anyhow::Result<String> {
    if is_local_feature(reference) {
        return Ok(reference.to_owned());
    }
    if !reference.contains('/') {
        bail!("Feature dependencies must use an explicit OCI reference or local path");
    }
    Ok(OciFeatureReference::parse(reference)?.canonical())
}

fn parse_short_reference(reference: &str) -> anyhow::Result<(&str, &str)> {
    if !reference.is_ascii()
        || reference.bytes().any(|byte| byte.is_ascii_whitespace())
        || reference.contains('@')
    {
        bail!("short Feature reference has invalid characters");
    }
    let (name, tag) = match reference.rsplit_once(':') {
        Some((name, "")) => {
            let _ = name;
            bail!("short Feature reference has an empty tag")
        }
        Some((name, tag)) => (name, tag),
        None => (reference, IMPLICIT_TAG),
    };
    validate_repository_component(name).context("short Feature reference has an invalid name")?;
    validate_tag(tag).context("short Feature reference has an invalid tag")?;
    Ok((name, tag))
}

fn validate_repository_path(path: &str) -> anyhow::Result<()> {
    if path.is_empty() {
        bail!("repository path is empty");
    }
    for component in path.split('/') {
        validate_repository_component(component)?;
    }
    Ok(())
}

fn validate_repository_component(component: &str) -> anyhow::Result<()> {
    let bytes = component.as_bytes();
    if bytes.is_empty() || !is_repository_alphanumeric(bytes[0]) {
        bail!("repository path components must start and end with a lowercase letter or digit");
    }

    let mut cursor = 0;
    while cursor < bytes.len() {
        while cursor < bytes.len() && is_repository_alphanumeric(bytes[cursor]) {
            cursor += 1;
        }
        if cursor == bytes.len() {
            return Ok(());
        }

        cursor = match bytes[cursor] {
            b'.' => cursor + 1,
            b'_' if bytes.get(cursor + 1) == Some(&b'_') => cursor + 2,
            b'_' => cursor + 1,
            b'-' => {
                let mut end = cursor + 1;
                while bytes.get(end) == Some(&b'-') {
                    end += 1;
                }
                end
            }
            _ => bail!("repository path contains an invalid character"),
        };
        if cursor == bytes.len() || !is_repository_alphanumeric(bytes[cursor]) {
            bail!("repository path contains an invalid separator sequence");
        }
    }
    Ok(())
}

fn is_repository_alphanumeric(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

fn validate_tag(tag: &str) -> anyhow::Result<()> {
    let bytes = tag.as_bytes();
    if bytes.is_empty() || bytes.len() > 128 {
        bail!("tag must contain between 1 and 128 ASCII characters");
    }
    if !bytes[0].is_ascii_alphanumeric() && bytes[0] != b'_' {
        bail!("tag has an invalid first character");
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        bail!("tag contains an invalid character");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn default_repository_validates_and_canonicalizes_authority() {
        let repository: DefaultFeatureRepository =
            "REGISTRY.example:443/team/features".parse().unwrap();
        assert_eq!(repository.to_string(), "registry.example/team/features");

        let repository: DefaultFeatureRepository =
            "[2001:db8::1]:5443/team/features".parse().unwrap();
        assert_eq!(repository.to_string(), "[2001:db8::1]:5443/team/features");
    }

    #[test]
    fn default_repository_rejects_url_tag_digest_and_malformed_paths() {
        for value in [
            "ghcr.io",
            "https://ghcr.io/team/features",
            "user@ghcr.io/team/features",
            "ghcr.io/team/features:1",
            "ghcr.io/team/features@sha256:abc",
            "ghcr.io/team/features/",
            "ghcr.io/team//features",
            "ghcr.io/team/../features",
            "ghcr.io/Team/features",
            "ghcr.io/team/${FEATURES}",
            "ghcr.io/team/features?x=1",
        ] {
            assert!(
                value.parse::<DefaultFeatureRepository>().is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn repository_components_accept_oci_separator_forms() {
        for value in [
            "ghcr.io/team/my.feature",
            "ghcr.io/team/my_feature",
            "ghcr.io/team/my__feature",
            "ghcr.io/team/my--feature",
        ] {
            assert!(value.parse::<DefaultFeatureRepository>().is_ok(), "{value}");
        }
        for value in [
            "ghcr.io/team/my._feature",
            "ghcr.io/team/my___feature",
            "ghcr.io/team/-feature",
            "ghcr.io/team/feature-",
        ] {
            assert!(
                value.parse::<DefaultFeatureRepository>().is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn project_reference_expands_short_names_and_tags() {
        let repository = "ghcr.io/example/features".parse().unwrap();
        assert_eq!(
            normalize_project_feature("sudo", Some(&repository)).unwrap(),
            "ghcr.io/example/features/sudo:latest"
        );
        assert_eq!(
            normalize_project_feature("sudo:1", Some(&repository)).unwrap(),
            "ghcr.io/example/features/sudo:1"
        );
    }

    #[test]
    fn project_reference_uses_builtin_default_when_unconfigured() {
        assert_eq!(
            normalize_project_feature("sudo", None).unwrap(),
            "ghcr.io/dc-powertools/features/sudo:latest"
        );
        assert_eq!(
            normalize_project_feature("sudo:1", None).unwrap(),
            "ghcr.io/dc-powertools/features/sudo:1"
        );
    }

    #[test]
    fn explicit_untagged_reference_uses_latest_and_local_stays_literal() {
        assert_eq!(
            normalize_project_feature("ghcr.io/devcontainers/features/node", None).unwrap(),
            "ghcr.io/devcontainers/features/node:latest"
        );
        assert_eq!(normalize_project_feature("./node", None).unwrap(), "./node");
    }

    #[test]
    fn malformed_short_references_are_rejected() {
        for value in ["", "sudo:", "Sudo", "sudo@sha256:abc", "sudo::1"] {
            assert!(normalize_project_feature(value, None).is_err(), "{value}");
        }
    }

    #[test]
    fn dependency_never_uses_project_shorthand() {
        assert!(normalize_feature_dependency("sudo:1").is_err());
        assert_eq!(
            normalize_feature_dependency("ghcr.io/team/features/sudo").unwrap(),
            "ghcr.io/team/features/sudo:latest"
        );
        assert_eq!(normalize_feature_dependency("../sudo").unwrap(), "../sudo");
    }

    #[test]
    fn project_shorthand_and_explicit_dependency_share_one_identity() {
        let repository = "ghcr.io/team/features".parse().unwrap();
        assert_eq!(
            normalize_project_feature("sudo", Some(&repository)).unwrap(),
            normalize_feature_dependency("ghcr.io/team/features/sudo").unwrap()
        );
    }

    #[test]
    fn invalid_reference_errors_do_not_echo_user_information_or_query_data() {
        const SECRET: &str = "sentinel-feature-reference-secret";
        let error = OciFeatureReference::parse(&format!(
            "user:{SECRET}@registry.example/owner/feature?query={SECRET}:1"
        ))
        .unwrap_err();
        assert!(!format!("{error:#}").contains(SECRET));
    }

    proptest! {
        #[test]
        fn arbitrary_project_references_do_not_panic(input in ".{0,256}") {
            let repository = "ghcr.io/team/features".parse().unwrap();
            let _ = normalize_project_feature(&input, Some(&repository));
        }
    }
}
