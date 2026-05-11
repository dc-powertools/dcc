use std::{fmt, path::PathBuf};

use crate::workspace::Workspace;

#[derive(Debug, Clone)]
pub(crate) struct ProfileName(String);

#[derive(Debug, Clone)]
pub(crate) struct ContainerName(String);

#[derive(Debug, Clone)]
pub(crate) struct ImageTag(String);

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

impl ContainerName {
    /// Derives container name as <workspace-basename>--<profile-name>.
    /// Falls back to "root" if workspace root has no basename (e.g. path is /).
    /// Per Decision 3 in the plan: returns Self, never Result.
    pub(crate) fn new(workspace: &Workspace, profile: &ProfileName) -> Self {
        let basename = workspace
            .root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root");
        Self(format!("{}--{}", basename, profile.0))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns an ImageTag with the same string.
    /// Docker image tags and container names are separate namespaces.
    pub(crate) fn as_image_tag(&self) -> ImageTag {
        ImageTag(self.0.clone())
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
        }
    }

    #[test]
    fn container_name_basic() {
        let ws = workspace("/home/user/my-project");
        let p = ProfileName::new("claude");
        assert_eq!(ContainerName::new(&ws, &p).as_str(), "my-project--claude");
    }

    #[test]
    fn container_name_default_profile() {
        let ws = workspace("/home/user/my-project");
        let p = ProfileName::new("devcontainer");
        assert_eq!(
            ContainerName::new(&ws, &p).as_str(),
            "my-project--devcontainer"
        );
    }

    #[test]
    fn container_name_root_fallback() {
        let ws = workspace("/");
        let p = ProfileName::new("dev");
        assert_eq!(ContainerName::new(&ws, &p).as_str(), "root--dev");
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
    fn as_image_tag_equals_container_name() {
        let ws = workspace("/home/user/project");
        let p = ProfileName::new("dev");
        let cn = ContainerName::new(&ws, &p);
        assert_eq!(cn.as_image_tag().as_str(), cn.as_str());
    }

    #[test]
    fn display_matches_inner_string() {
        let p = ProfileName::new("claude");
        assert_eq!(format!("{}", p), "claude");
    }
}
