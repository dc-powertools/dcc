use std::path::Path;

use anyhow::Context as _;

use super::oci::DownloadedFeature;

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

    Ok(DownloadedFeature {
        install_sh,
        feature_json,
        env,
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
