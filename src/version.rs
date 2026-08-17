use anyhow::{bail, Context as _};
use std::cmp::Ordering;

use crate::docker;

/// Semantic version used for the host↔supervisor compatibility gate. The
/// `dcc.version` image label records the dcc that built the image; the CLI
/// compares it against its own version to decide whether the image's baked
/// supervisor is protocol-compatible.
///
/// Compatibility rule (decision 0004 Q1): equal or patch-only drift is
/// compatible; major or minor drift, or a missing label, is incompatible.
/// This rests on a maintenance rule: a patch release must not change the
/// host↔supervisor protocol (the `dcc-ctl` verbs, the `dcc-exec` registration
/// contract, and exit codes 252/253). Any protocol change requires at least a
/// minor bump.
#[derive(Debug, Clone, Eq, PartialEq)]
struct SemVer {
    major: u64,
    minor: u64,
    patch: u64,
}

impl SemVer {
    fn parse(s: &str) -> anyhow::Result<Self> {
        let s = s.trim().trim_start_matches('v');
        let mut parts = s.split('.');
        let major = parts
            .next()
            .context("version has no major component")?
            .parse::<u64>()
            .with_context(|| format!("invalid major version in `{s}`"))?;
        let minor = parts
            .next()
            .context("version has no minor component")?
            .parse::<u64>()
            .with_context(|| format!("invalid minor version in `{s}`"))?;
        // Patch may carry pre-release/build metadata; take the leading numeric
        // run and ignore the rest for compatibility comparison.
        let patch_raw = parts.next().context("version has no patch component")?;
        let patch_str = patch_raw
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .filter(|s| !s.is_empty())
            .with_context(|| format!("invalid patch version in `{s}`"))?;
        let patch = patch_str
            .parse::<u64>()
            .with_context(|| format!("invalid patch version in `{s}`"))?;
        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    /// Compatible iff major and minor are equal (patch drift is allowed).
    fn compatible_with(&self, other: &Self) -> bool {
        self.major == other.major && self.minor == other.minor
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then(self.minor.cmp(&other.minor))
            .then(self.patch.cmp(&other.patch))
    }
}

/// Checks the image's `dcc.version` label against the current dcc version and
/// returns an error if the image is incompatible (major or minor drift, or a
/// missing label). Patch-only drift is compatible and proceeds silently.
/// `dcc build` is exempt and should not call this — it is the command that
/// fixes incompatibility.
pub(crate) async fn ensure_image_version_compatible(
    image: &str,
    profile_arg: &str,
    strict: bool,
) -> anyhow::Result<()> {
    let image_version = docker::inspect_image_dcc_version(image)
        .await
        .with_context(|| format!("failed to inspect dcc version label on image `{image}`"))?;
    match image_version.as_deref() {
        Some(v) => {
            let image_ver = SemVer::parse(v)
                .with_context(|| format!("invalid dcc.version label `{v}` on image `{image}`"))?;
            let current_ver =
                SemVer::parse(env!("CARGO_PKG_VERSION")).context("invalid CARGO_PKG_VERSION")?;
            if !image_ver.compatible_with(&current_ver) {
                bail!(
                    "image `{image}` was built with dcc {v}, which is incompatible with \
                     current dcc {current}; rebuild the image with `{rebuild}`",
                    current = env!("CARGO_PKG_VERSION"),
                    rebuild = rebuild_command(profile_arg, strict),
                );
            }
            Ok(())
        }
        None => bail!(
            "image `{image}` does not record the dcc version it was built with; \
             it may predate dcc's version stamping or was not built by dcc. \
             Rebuild the image with `{rebuild}`",
            rebuild = rebuild_command(profile_arg, strict),
        ),
    }
}

/// Best-effort variant for the `stop` path: a version incompatibility must not
/// prevent stopping a container.
pub(crate) async fn ensure_image_version_compatible_best_effort(
    image: &str,
    profile_arg: &str,
    strict: bool,
) {
    let _ = ensure_image_version_compatible(image, profile_arg, strict).await;
}

pub(crate) fn rebuild_command(profile_arg: &str, strict: bool) -> String {
    let mut parts = vec!["dcc".to_string()];
    if strict {
        parts.push("--strict".to_string());
    }
    if profile_arg != "devcontainer" {
        parts.push("-p".to_string());
        parts.push(shell_arg(profile_arg));
    }
    parts.push("build".to_string());
    parts.join(" ")
}

fn shell_arg(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b':'))
    {
        return arg.to_string();
    }

    let quoted = arg.replace('\'', "'\\''");
    format!("'{quoted}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parse_basic() {
        assert_eq!(
            SemVer::parse("1.2.3").unwrap(),
            SemVer {
                major: 1,
                minor: 2,
                patch: 3
            }
        );
    }

    #[test]
    fn semver_parse_strips_v_prefix() {
        assert_eq!(
            SemVer::parse("v1.2.3").unwrap(),
            SemVer {
                major: 1,
                minor: 2,
                patch: 3
            }
        );
    }

    #[test]
    fn semver_parse_ignores_prerelease() {
        let v = SemVer::parse("1.2.3-rc1").unwrap();
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn semver_parse_ignores_build_metadata() {
        let v = SemVer::parse("1.2.3+build.7").unwrap();
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn semver_compatible_when_equal() {
        let a = SemVer::parse("1.2.3").unwrap();
        assert!(a.compatible_with(&a));
    }

    #[test]
    fn semver_compatible_when_patch_differs() {
        let a = SemVer::parse("1.2.3").unwrap();
        let b = SemVer::parse("1.2.9").unwrap();
        assert!(a.compatible_with(&b));
    }

    #[test]
    fn semver_incompatible_when_minor_differs() {
        let a = SemVer::parse("1.2.3").unwrap();
        let b = SemVer::parse("1.3.0").unwrap();
        assert!(!a.compatible_with(&b));
    }

    #[test]
    fn semver_incompatible_when_major_differs() {
        let a = SemVer::parse("1.2.3").unwrap();
        let b = SemVer::parse("2.0.0").unwrap();
        assert!(!a.compatible_with(&b));
    }

    #[test]
    fn semver_parse_rejects_missing_minor() {
        assert!(SemVer::parse("1").is_err());
    }

    #[test]
    fn semver_parse_rejects_non_numeric() {
        assert!(SemVer::parse("a.b.c").is_err());
    }

    #[test]
    fn rebuild_command_default_profile() {
        assert_eq!(rebuild_command("devcontainer", false), "dcc build");
    }

    #[test]
    fn rebuild_command_named_profile_uses_short_flag() {
        assert_eq!(rebuild_command("base", false), "dcc -p base build");
    }

    #[test]
    fn rebuild_command_preserves_strict() {
        assert_eq!(rebuild_command("base", true), "dcc --strict -p base build");
    }

    #[test]
    fn rebuild_command_quotes_profile_with_spaces() {
        assert_eq!(
            rebuild_command("./profiles/rust dev.json", false),
            "dcc -p './profiles/rust dev.json' build"
        );
    }

    #[test]
    fn rebuild_command_quotes_single_quote() {
        assert_eq!(
            rebuild_command("./profiles/bob's.json", false),
            "dcc -p './profiles/bob'\\''s.json' build"
        );
    }
}
