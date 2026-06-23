use anyhow::Context as _;

use crate::docker;

pub(crate) async fn warn_if_image_version_mismatch(
    image: &str,
    current_uses_fast_path: Option<bool>,
    profile_arg: &str,
    strict: bool,
) -> anyhow::Result<()> {
    let image_version = docker::inspect_image_dcc_version(image)
        .await
        .with_context(|| format!("failed to inspect dcc version label on image `{image}`"))?;
    if let Some(warning) = version_warning(
        image,
        image_version.as_deref(),
        current_uses_fast_path,
        &rebuild_command(profile_arg, strict),
    ) {
        eprintln!("{warning}");
    }
    Ok(())
}

pub(crate) async fn warn_if_image_version_mismatch_best_effort(
    image: &str,
    current_uses_fast_path: Option<bool>,
    profile_arg: &str,
    strict: bool,
) {
    let _ =
        warn_if_image_version_mismatch(image, current_uses_fast_path, profile_arg, strict).await;
}

pub(crate) fn version_warning(
    image: &str,
    image_version: Option<&str>,
    current_uses_fast_path: Option<bool>,
    rebuild_command: &str,
) -> Option<String> {
    let current = env!("CARGO_PKG_VERSION");
    match image_version {
        Some(version) if version == current => None,
        Some(version) => Some(format!(
            "warning: image `{image}` was built with dcc {version}, but current dcc is {current}; \
             rebuild the image with `{rebuild_command}`"
        )),
        None if current_uses_fast_path == Some(true) => None,
        None if current_uses_fast_path == Some(false) => Some(format!(
            "warning: image `{image}` does not record the dcc version it was built with; \
             rebuild the image with `{rebuild_command}`"
        )),
        None => None,
    }
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
    fn version_warning_none_when_versions_match() {
        assert_eq!(
            version_warning(
                "img",
                Some(env!("CARGO_PKG_VERSION")),
                Some(false),
                "dcc build"
            ),
            None
        );
    }

    #[test]
    fn version_warning_reports_explicit_mismatch() {
        let warning = version_warning("img", Some("0.0.1"), Some(true), "dcc build").unwrap();
        assert!(warning.contains("built with dcc 0.0.1"), "{warning}");
        assert!(warning.contains(env!("CARGO_PKG_VERSION")), "{warning}");
        assert!(warning.contains("dcc build"), "{warning}");
    }

    #[test]
    fn version_warning_reports_missing_label_for_full_build() {
        let warning = version_warning("img", None, Some(false), "dcc build").unwrap();
        assert!(warning.contains("does not record"), "{warning}");
        assert!(warning.contains("dcc build"), "{warning}");
    }

    #[test]
    fn version_warning_suppresses_missing_label_for_fast_path() {
        assert_eq!(version_warning("img", None, Some(true), "dcc build"), None);
    }

    #[test]
    fn version_warning_suppresses_missing_label_when_current_config_unknown() {
        assert_eq!(version_warning("img", None, None, "dcc build"), None);
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
