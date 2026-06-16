use std::path::Path;

use anyhow::Context as _;
use sha2::{Digest as _, Sha256};

use super::oci::DownloadedFeature;

/// Returns the Unix permission bits for `path`, or 0o644 on error or non-Unix platforms.
fn file_mode(path: &Path) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path)
            .map(|m| m.permissions().mode())
            .unwrap_or(0o644)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        0o644
    }
}

/// Recursively collects all regular files under `feature_dir` except the two
/// that are handled separately (`install.sh` and `devcontainer-feature.json`).
/// Returns `(relative_path, content, unix_mode)` triples where `relative_path`
/// is relative to `feature_dir` (e.g. `"library_scripts/common.sh"`), preserving
/// the directory structure so that `install.sh` can reference helpers by the
/// same relative paths it uses on disk.
/// Symlinks are skipped.
fn collect_extra_files(feature_dir: &Path) -> anyhow::Result<Vec<(String, Vec<u8>, u32)>> {
    let mut extra = Vec::new();
    collect_recursive(feature_dir, feature_dir, &mut extra)?;
    Ok(extra)
}

fn collect_recursive(
    feature_root: &Path,
    dir: &Path,
    extra: &mut Vec<(String, Vec<u8>, u32)>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("failed to read directory `{}`", dir.display()))?
    {
        let entry =
            entry.with_context(|| format!("failed to read entry in `{}`", dir.display()))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(feature_root)
            .expect("entries are within feature_root")
            .to_string_lossy()
            .into_owned();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to get file type for `{rel}`"))?;

        if file_type.is_dir() {
            collect_recursive(feature_root, &path, extra)?;
        } else if file_type.is_file() {
            if rel == "install.sh" || rel == "devcontainer-feature.json" {
                continue;
            }
            let content = std::fs::read(&path)
                .with_context(|| format!("failed to read `{rel}` from local feature"))?;
            extra.push((rel, content, file_mode(&path)));
        }
        // Symlinks are intentionally skipped.
    }
    Ok(())
}

/// Loads a feature from a local directory on disk.
///
/// `reference` is a relative path (starting with `./` or `../`) resolved relative
/// to `config_dir` — the directory containing the devcontainer config file, per the
/// devcontainer spec. The directory must contain `install.sh`; `devcontainer-feature.json`
/// is optional and used only to read option defaults.
pub(super) fn load_local_feature(
    reference: &str,
    config_dir: &Path,
    user_options: &serde_json::Value,
) -> anyhow::Result<DownloadedFeature> {
    let feature_dir = config_dir.join(reference);

    let install_sh = std::fs::read(feature_dir.join("install.sh")).with_context(|| {
        format!(
            "failed to read `install.sh` from local feature `{}`",
            feature_dir.display()
        )
    })?;

    let feature_json = std::fs::read(feature_dir.join("devcontainer-feature.json")).ok();

    let env = super::build_env(feature_json.as_deref(), user_options);
    let extra_files = collect_extra_files(&feature_dir).with_context(|| {
        format!(
            "failed to collect files from local feature `{}`",
            feature_dir.display()
        )
    })?;
    let resolved_digest = format!("sha256:{:x}", Sha256::digest(&install_sh));

    Ok(DownloadedFeature {
        install_sh,
        feature_json,
        env,
        extra_files,
        resolved_digest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn write(dir: &Path, name: &str, content: &[u8]) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn loads_install_sh() {
        let tmp = tempfile::tempdir().unwrap();
        let feature_dir = tmp.path().join("my-feature");
        std::fs::create_dir(&feature_dir).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh\necho hello\n");

        let result = load_local_feature("./my-feature", tmp.path(), &json!({})).unwrap();
        assert_eq!(result.install_sh, b"#!/bin/sh\necho hello\n");
        assert!(result.feature_json.is_none());
        assert!(result.env.is_empty());
    }

    #[test]
    fn loads_feature_json_and_applies_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let feature_dir = tmp.path().join("my-feature");
        std::fs::create_dir(&feature_dir).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh");
        write(
            &feature_dir,
            "devcontainer-feature.json",
            br#"{"options": {"version": {"default": "lts"}}}"#,
        );

        let result = load_local_feature("./my-feature", tmp.path(), &json!({})).unwrap();
        assert_eq!(result.env.get("VERSION").map(String::as_str), Some("lts"));
        assert!(result.feature_json.is_some());
    }

    #[test]
    fn user_options_override_defaults() {
        let tmp = tempfile::tempdir().unwrap();
        let feature_dir = tmp.path().join("feat");
        std::fs::create_dir(&feature_dir).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh");
        write(
            &feature_dir,
            "devcontainer-feature.json",
            br#"{"options": {"version": {"default": "lts"}}}"#,
        );

        let result = load_local_feature("./feat", tmp.path(), &json!({"version": "20"})).unwrap();
        assert_eq!(result.env.get("VERSION").map(String::as_str), Some("20"));
    }

    #[test]
    fn missing_install_sh_errors() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("feat")).unwrap();

        let err = load_local_feature("./feat", tmp.path(), &json!({})).unwrap_err();
        assert!(
            err.to_string().contains("install.sh"),
            "error should mention install.sh; got: {err}"
        );
    }

    #[test]
    fn missing_feature_dir_errors() {
        let tmp = tempfile::tempdir().unwrap();

        let err = load_local_feature("./nonexistent", tmp.path(), &json!({})).unwrap_err();
        assert!(
            err.to_string().contains("install.sh") || err.to_string().contains("nonexistent"),
            "error should mention the missing path; got: {err}"
        );
    }

    #[test]
    fn parent_relative_path() {
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("subdir");
        std::fs::create_dir(&config_dir).unwrap();
        let feature_dir = tmp.path().join("shared-feature");
        std::fs::create_dir(&feature_dir).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh\n");

        let result = load_local_feature("../shared-feature", &config_dir, &json!({})).unwrap();
        assert_eq!(result.install_sh, b"#!/bin/sh\n");
    }

    #[test]
    fn extra_files_are_collected() {
        let tmp = tempfile::tempdir().unwrap();
        let feature_dir = tmp.path().join("feat");
        std::fs::create_dir(&feature_dir).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh\n./helper.sh\n");
        write(&feature_dir, "helper.sh", b"#!/bin/sh\necho hello\n");

        let result = load_local_feature("./feat", tmp.path(), &json!({})).unwrap();
        assert_eq!(result.extra_files.len(), 1);
        let (name, content, _mode) = &result.extra_files[0];
        assert_eq!(name, "helper.sh");
        assert_eq!(content.as_slice(), b"#!/bin/sh\necho hello\n");
    }

    #[test]
    fn extra_files_include_subdirectory_contents() {
        let tmp = tempfile::tempdir().unwrap();
        let feature_dir = tmp.path().join("feat");
        std::fs::create_dir(&feature_dir).unwrap();
        std::fs::create_dir(feature_dir.join("library_scripts")).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh");
        write(
            &feature_dir.join("library_scripts"),
            "common.sh",
            b"#!/bin/sh\necho common\n",
        );

        let result = load_local_feature("./feat", tmp.path(), &json!({})).unwrap();
        assert_eq!(result.extra_files.len(), 1);
        assert_eq!(result.extra_files[0].0, "library_scripts/common.sh");
        assert_eq!(result.extra_files[0].1, b"#!/bin/sh\necho common\n");
    }

    #[test]
    fn install_sh_and_feature_json_not_in_extra_files() {
        let tmp = tempfile::tempdir().unwrap();
        let feature_dir = tmp.path().join("feat");
        std::fs::create_dir(&feature_dir).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh");
        write(&feature_dir, "devcontainer-feature.json", br#"{}"#);

        let result = load_local_feature("./feat", tmp.path(), &json!({})).unwrap();
        assert!(
            result.extra_files.is_empty(),
            "install.sh and devcontainer-feature.json must not appear in extra_files"
        );
    }

    #[test]
    fn missing_devcontainer_feature_json_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let feature_dir = tmp.path().join("feat");
        std::fs::create_dir(&feature_dir).unwrap();
        write(&feature_dir, "install.sh", b"#!/bin/sh");
        // no devcontainer-feature.json

        let result = load_local_feature("./feat", tmp.path(), &json!({})).unwrap();
        assert!(result.feature_json.is_none());
    }
}
