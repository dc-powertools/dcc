use std::path::PathBuf;

use anyhow::Context as _;

use crate::{
    config::{StateEntry, StateKind},
    profile::ProfileName,
    workspace::Workspace,
};

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

    pub(crate) fn plan_state_mounts(&self, state: &[StateEntry]) -> Vec<StateMount> {
        state
            .iter()
            .map(|entry| StateMount {
                host_path: state_host_path(&self.host_path, &entry.path),
                container_path: entry.path.clone(),
                kind: entry.kind,
            })
            .collect()
    }

    pub(crate) fn prepare_state_mounts(&self, mounts: &[StateMount]) -> anyhow::Result<()> {
        for mount in mounts {
            mount.prepare()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct StateMount {
    pub(crate) host_path: PathBuf,
    pub(crate) container_path: String,
    pub(crate) kind: StateKind,
}

impl StateMount {
    pub(crate) fn to_mount_arg(&self) -> String {
        format!(
            "type=bind,src={},dst={}",
            self.host_path.display(),
            self.container_path
        )
    }

    fn prepare(&self) -> anyhow::Result<()> {
        match self.kind {
            StateKind::Directory => self.prepare_directory(),
            StateKind::File => self.prepare_file(),
        }
    }

    fn prepare_directory(&self) -> anyhow::Result<()> {
        if self.host_path.exists() && !self.host_path.is_dir() {
            anyhow::bail!(
                "state path `{}` maps to `{}`, which exists but is not a directory",
                self.container_path,
                self.host_path.display()
            );
        }
        std::fs::create_dir_all(&self.host_path).with_context(|| {
            format!(
                "failed to create state directory `{}` for `{}`",
                self.host_path.display(),
                self.container_path
            )
        })
    }

    fn prepare_file(&self) -> anyhow::Result<()> {
        if self.host_path.exists() {
            if self.host_path.is_file() {
                return Ok(());
            }
            anyhow::bail!(
                "state path `{}` maps to `{}`, which exists but is not a file",
                self.container_path,
                self.host_path.display()
            );
        }
        let parent = self.host_path.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "state file `{}` has no host parent directory",
                self.host_path.display()
            )
        })?;
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create parent directory `{}` for state file `{}`",
                parent.display(),
                self.container_path
            )
        })?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.host_path)
            .with_context(|| {
                format!(
                    "failed to create state file `{}` for `{}`",
                    self.host_path.display(),
                    self.container_path
                )
            })?;
        Ok(())
    }
}

fn state_host_path(cache_root: &std::path::Path, container_path: &str) -> PathBuf {
    let mut host_path = cache_root.join("state");
    for segment in container_path.trim_start_matches('/').split('/') {
        if !segment.is_empty() {
            host_path.push(segment);
        }
    }
    host_path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::{profile::ProfileName, workspace::Workspace};

    fn ws(path: &str) -> Workspace {
        Workspace {
            root: PathBuf::from(path),
            identity: path.to_string(),
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
            identity: dir.path().to_string_lossy().into_owned(),
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
            identity: dir.path().to_string_lossy().into_owned(),
        };
        let cache = CacheDir::new(&ws, &ProfileName::new("profile"));
        assert!(!dir.path().join(".dcc").exists());
        cache.ensure_exists().unwrap();
        assert!(dir.path().join(".dcc").is_dir());
        assert!(cache.host_path.is_dir());
    }

    #[test]
    fn state_mount_plan_is_rooted_below_profile_cache() {
        let cache = CacheDir {
            host_path: PathBuf::from("/workspace/.dcc/dev"),
        };
        let mounts = cache.plan_state_mounts(&[StateEntry {
            path: "/home/dev/.cache".to_string(),
            kind: StateKind::Directory,
        }]);
        assert_eq!(
            mounts,
            vec![StateMount {
                host_path: PathBuf::from("/workspace/.dcc/dev/state/home/dev/.cache"),
                container_path: "/home/dev/.cache".to_string(),
                kind: StateKind::Directory,
            }]
        );
        assert_eq!(
            mounts[0].to_mount_arg(),
            "type=bind,src=/workspace/.dcc/dev/state/home/dev/.cache,dst=/home/dev/.cache"
        );
    }

    #[test]
    fn prepare_state_mounts_creates_directory_state() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheDir {
            host_path: dir.path().join(".dcc/dev"),
        };
        let mounts = cache.plan_state_mounts(&[StateEntry {
            path: "/home/dev/.cargo".to_string(),
            kind: StateKind::Directory,
        }]);
        cache.prepare_state_mounts(&mounts).unwrap();
        assert!(mounts[0].host_path.is_dir());
    }

    #[test]
    fn prepare_state_mounts_creates_file_state_and_parent() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheDir {
            host_path: dir.path().join(".dcc/dev"),
        };
        let mounts = cache.plan_state_mounts(&[StateEntry {
            path: "/home/dev/.npmrc".to_string(),
            kind: StateKind::File,
        }]);
        cache.prepare_state_mounts(&mounts).unwrap();
        assert!(mounts[0].host_path.is_file());
        assert!(mounts[0].host_path.parent().unwrap().is_dir());
    }

    #[test]
    fn prepare_state_mounts_rejects_file_when_directory_exists() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CacheDir {
            host_path: dir.path().join(".dcc/dev"),
        };
        let mounts = cache.plan_state_mounts(&[StateEntry {
            path: "/home/dev/.npmrc".to_string(),
            kind: StateKind::File,
        }]);
        std::fs::create_dir_all(&mounts[0].host_path).unwrap();
        let err = cache.prepare_state_mounts(&mounts).unwrap_err();
        assert!(
            err.to_string().contains("not a file"),
            "expected file-kind error, got: {err:#}"
        );
    }
}
