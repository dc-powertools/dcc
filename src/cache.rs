use std::path::PathBuf;

use anyhow::Context as _;

use crate::{profile::ProfileName, workspace::Workspace};

#[derive(Debug)]
pub(crate) struct CacheDir {
    pub(crate) host_path: PathBuf,
}

impl CacheDir {
    /// Cache directory is at <workspace.root>/.dcc/<profile-name>/
    pub(crate) fn new(workspace: &Workspace, profile: &ProfileName) -> Self {
        Self {
            host_path: workspace.root.join(".dcc").join(profile.as_str()),
        }
    }

    /// Creates the cache directory (and any missing intermediate dirs).
    /// Idempotent: succeeds if the directory already exists.
    pub(crate) fn ensure_exists(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.host_path).with_context(|| {
            format!(
                "failed to create cache directory `{}`",
                self.host_path.display()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::{profile::ProfileName, workspace::Workspace};

    fn ws(path: &str) -> Workspace {
        Workspace {
            root: PathBuf::from(path),
        }
    }

    #[test]
    fn host_path_correct() {
        let cache = CacheDir::new(&ws("/home/user/project"), &ProfileName::new("claude"));
        assert_eq!(
            cache.host_path,
            PathBuf::from("/home/user/project/.dcc/claude")
        );
    }

    #[test]
    fn host_path_is_absolute() {
        let cache = CacheDir::new(&ws("/some/abs/path"), &ProfileName::new("dev"));
        assert!(cache.host_path.is_absolute());
    }

    #[test]
    fn ensure_exists_creates_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: dir.path().to_path_buf(),
        };
        let cache = CacheDir::new(&ws, &ProfileName::new("test"));
        cache.ensure_exists().expect("first call failed");
        assert!(cache.host_path.is_dir());
        cache
            .ensure_exists()
            .expect("second call failed (idempotency)");
    }

    #[test]
    fn ensure_exists_creates_intermediate_dcc_dir() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace {
            root: dir.path().to_path_buf(),
        };
        let cache = CacheDir::new(&ws, &ProfileName::new("profile"));
        assert!(!dir.path().join(".dcc").exists());
        cache.ensure_exists().unwrap();
        assert!(dir.path().join(".dcc").is_dir());
        assert!(cache.host_path.is_dir());
    }
}
