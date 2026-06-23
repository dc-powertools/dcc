use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::workspace::Workspace;

#[derive(Debug, Clone)]
pub(crate) struct ProfileName(String);

#[derive(Debug, Clone)]
pub(crate) struct ContainerId(String);

#[derive(Debug, Clone)]
pub(crate) struct ImageTag(String);

#[derive(Debug, Clone)]
pub(crate) struct ContainerName(String);

impl ProfileName {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns .devcontainer/<name>.json relative to workspace root.
    /// No special-casing: "devcontainer" → .devcontainer/devcontainer.json by the same rule.
    pub(crate) fn config_path(&self, workspace: &Workspace) -> PathBuf {
        workspace
            .root
            .join(".devcontainer")
            .join(format!("{}.json", self.0))
    }
}

/// Derives a profile name from the canonicalized path to a config file.
///
/// If `config` falls within the workspace, the name is derived from the path
/// relative to the workspace root. Otherwise it is derived from the absolute
/// path. In both cases non-alphanumeric characters are replaced with `-`,
/// the result is lowercased, and leading/trailing `-` are stripped.
///
/// Examples (workspace root `/proj`):
///   `/proj/.devcontainer/claude.json` → `devcontainer-claude-json`
///   `/proj/configs/dev.json`          → `configs-dev-json`
///   `/shared/base.json`               → `shared-base-json`
pub(crate) fn path_to_profile_name(config: &Path, workspace: &Workspace) -> ProfileName {
    let path_str = if config.starts_with(&workspace.root) {
        config
            .strip_prefix(&workspace.root)
            .expect("starts_with checked above")
            .to_string_lossy()
            .into_owned()
    } else {
        config.to_string_lossy().into_owned()
    };
    let slug: String = path_str
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    ProfileName::new(slug)
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ProfileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ContainerId {
    /// Derives a stable dcc container id from the workspace identity and profile.
    ///
    /// Format: `dcc-<12hex>--<profile>` where `<12hex>` is the first 6 bytes of
    /// the SHA-256 of the workspace identity string rendered as lowercase hex.
    /// The identity is the git `origin` remote URL when available, falling back
    /// to the canonical workspace root path — so the id is identical on every
    /// machine that clones the same repository.
    pub(crate) fn new(workspace: &Workspace, profile: &ProfileName) -> Self {
        use sha2::{Digest as _, Sha256};
        let hash = Sha256::digest(workspace.identity.as_bytes());
        let hex = format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5]
        );
        Self(format!("dcc-{}--{}", hex, profile.0))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns an ImageTag with the same string. Docker image tags and container
    /// ids are separate namespaces.
    pub(crate) fn as_image_tag(&self) -> ImageTag {
        ImageTag(self.0.clone())
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContainerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ContainerName {
    pub(crate) fn resolve(configured_name: Option<&str>, fallback: &ContainerId) -> Self {
        let Some(configured_name) = configured_name else {
            return Self(fallback.0.clone());
        };
        let Some(sanitized) = sanitize_container_name(configured_name) else {
            eprintln!(
                "warning: devcontainer name `{}` cannot be used as a Docker container name; \
                 using `{}` instead",
                configured_name.trim(),
                fallback.as_str()
            );
            return Self(fallback.0.clone());
        };

        if configured_name.trim() != sanitized {
            eprintln!(
                "warning: devcontainer name `{}` was converted to Docker container name `{}`",
                configured_name.trim(),
                sanitized
            );
        }
        Self(sanitized)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn sanitize_container_name(name: &str) -> Option<String> {
    let mut result = String::new();
    let mut previous_dash = false;
    for ch in name.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-') {
            ch
        } else {
            '-'
        };
        if mapped == '-' {
            if previous_dash {
                continue;
            }
            previous_dash = true;
        } else {
            previous_dash = false;
        }
        result.push(mapped);
    }

    let trimmed = result
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric())
        .to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

impl fmt::Display for ContainerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContainerName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ImageTag {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ImageTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ImageTag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace(path: &str) -> Workspace {
        Workspace {
            root: PathBuf::from(path),
            identity: path.to_string(),
        }
    }

    fn expected_hex(identity: &str) -> String {
        use sha2::{Digest as _, Sha256};
        let hash = Sha256::digest(identity.as_bytes());
        format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5]
        )
    }

    #[test]
    fn container_id_basic() {
        let ws = workspace("/home/user/my-project");
        let p = ProfileName::new("claude");
        let hex = expected_hex("/home/user/my-project");
        assert_eq!(
            ContainerId::new(&ws, &p).as_str(),
            format!("dcc-{hex}--claude")
        );
    }

    #[test]
    fn container_id_default_profile() {
        let ws = workspace("/home/user/my-project");
        let p = ProfileName::new("devcontainer");
        let hex = expected_hex("/home/user/my-project");
        assert_eq!(
            ContainerId::new(&ws, &p).as_str(),
            format!("dcc-{hex}--devcontainer")
        );
    }

    #[test]
    fn container_id_root_fallback() {
        let ws = workspace("/");
        let p = ProfileName::new("dev");
        let hex = expected_hex("/");
        assert_eq!(
            ContainerId::new(&ws, &p).as_str(),
            format!("dcc-{hex}--dev")
        );
    }

    #[test]
    fn container_id_dcc_prefix() {
        let ws = workspace("/any/path");
        let p = ProfileName::new("test");
        assert!(ContainerId::new(&ws, &p).as_str().starts_with("dcc-"));
    }

    #[test]
    fn container_id_profile_in_suffix() {
        let ws = workspace("/any/path");
        let p = ProfileName::new("myprofile");
        assert!(ContainerId::new(&ws, &p).as_str().ends_with("--myprofile"));
    }

    #[test]
    fn container_id_same_identity_same_id() {
        let ws1 = Workspace {
            root: PathBuf::from("/path/a"),
            identity: "https://github.com/org/repo".to_string(),
        };
        let ws2 = Workspace {
            root: PathBuf::from("/completely/different/path/b"),
            identity: "https://github.com/org/repo".to_string(),
        };
        let p = ProfileName::new("dev");
        assert_eq!(
            ContainerId::new(&ws1, &p).as_str(),
            ContainerId::new(&ws2, &p).as_str(),
            "same identity must produce the same container id regardless of root path"
        );
    }

    #[test]
    fn container_id_different_identity_different_id() {
        let ws1 = workspace("/home/user/project-a");
        let ws2 = workspace("/home/user/project-b");
        let p = ProfileName::new("dev");
        assert_ne!(
            ContainerId::new(&ws1, &p).as_str(),
            ContainerId::new(&ws2, &p).as_str(),
        );
    }

    #[test]
    fn config_path_basic() {
        let ws = workspace("/home/user/project");
        let p = ProfileName::new("claude");
        assert_eq!(
            p.config_path(&ws),
            PathBuf::from("/home/user/project/.devcontainer/claude.json")
        );
    }

    #[test]
    fn config_path_default_profile_no_special_case() {
        let ws = workspace("/home/user/project");
        let p = ProfileName::new("devcontainer");
        assert_eq!(
            p.config_path(&ws),
            PathBuf::from("/home/user/project/.devcontainer/devcontainer.json")
        );
    }

    #[test]
    fn as_image_tag_equals_container_id() {
        let ws = workspace("/home/user/project");
        let p = ProfileName::new("dev");
        let cn = ContainerId::new(&ws, &p);
        assert_eq!(cn.as_image_tag().as_str(), cn.as_str());
    }

    #[test]
    fn container_name_uses_configured_name() {
        let id = ContainerId("dcc-abc123--dev".to_string());
        let name = ContainerName::resolve(Some("example"), &id);
        assert_eq!(name.as_str(), "example");
    }

    #[test]
    fn container_name_falls_back_to_id_when_missing() {
        let id = ContainerId("dcc-abc123--dev".to_string());
        let name = ContainerName::resolve(None, &id);
        assert_eq!(name.as_str(), id.as_str());
    }

    #[test]
    fn sanitize_container_name_converts_invalid_chars() {
        assert_eq!(
            sanitize_container_name("example/project app"),
            Some("example-project-app".to_string())
        );
    }

    #[test]
    fn sanitize_container_name_keeps_valid_chars() {
        assert_eq!(
            sanitize_container_name("example_project.dev-1"),
            Some("example_project.dev-1".to_string())
        );
    }

    #[test]
    fn sanitize_container_name_trims_invalid_edges() {
        assert_eq!(
            sanitize_container_name(" --.example.-- "),
            Some("example".to_string())
        );
    }

    #[test]
    fn sanitize_container_name_collapses_repeated_dashes() {
        assert_eq!(
            sanitize_container_name("example///project"),
            Some("example-project".to_string())
        );
    }

    #[test]
    fn container_name_falls_back_when_sanitized_name_is_empty() {
        let id = ContainerId("dcc-abc123--dev".to_string());
        let name = ContainerName::resolve(Some(" /// "), &id);
        assert_eq!(name.as_str(), id.as_str());
    }

    #[test]
    fn display_matches_inner_string() {
        let p = ProfileName::new("claude");
        assert_eq!(format!("{}", p), "claude");
    }

    // --- path_to_profile_name ---

    #[test]
    fn path_name_inside_workspace() {
        let ws = workspace("/proj");
        let config = PathBuf::from("/proj/configs/dev.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "configs-dev-json"
        );
    }

    #[test]
    fn path_name_in_devcontainer_dir() {
        // The leading '.' of '.devcontainer' becomes '-', which is then trimmed,
        // so the result is "devcontainer-claude-json" not "-devcontainer-claude-json".
        let ws = workspace("/proj");
        let config = PathBuf::from("/proj/.devcontainer/claude.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "devcontainer-claude-json"
        );
    }

    #[test]
    fn path_name_outside_workspace() {
        let ws = workspace("/proj");
        let config = PathBuf::from("/shared/configs/base.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "shared-configs-base-json"
        );
    }

    #[test]
    fn path_name_nested_inside_workspace() {
        let ws = workspace("/home/user/myproject");
        let config = PathBuf::from("/home/user/myproject/a/b/c.json");
        assert_eq!(path_to_profile_name(&config, &ws).as_str(), "a-b-c-json");
    }

    #[test]
    fn path_name_special_chars_replaced() {
        let ws = workspace("/proj");
        let config = PathBuf::from("/proj/my.config/dev-2.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "my-config-dev-2-json"
        );
    }
}
