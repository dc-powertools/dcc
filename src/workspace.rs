use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

#[derive(Debug)]
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
}

pub(crate) fn find_workspace() -> anyhow::Result<Workspace> {
    let start = std::env::current_dir().context("failed to determine current working directory")?;
    find_workspace_from(&start)
}

fn find_workspace_from(start: &Path) -> anyhow::Result<Workspace> {
    let mut path = fs::canonicalize(start)
        .with_context(|| format!("failed to canonicalize path: {}", start.display()))?;

    if path.file_name() == Some(std::ffi::OsStr::new(".devcontainer")) {
        path = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("`.devcontainer` has no parent directory"))?
            .to_path_buf();
    }

    for dir in std::iter::once(path.as_path()).chain(path.ancestors().skip(1)) {
        if dir.join(".devcontainer").is_dir() {
            return Ok(Workspace {
                root: dir.to_path_buf(),
            });
        }
    }

    anyhow::bail!(
        "could not find `.devcontainer/` directory in `{}` or any of its ancestors",
        path.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn from_workspace_root() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join(".devcontainer")).unwrap();

        let ws = find_workspace_from(root).unwrap();
        assert_eq!(ws.root, fs::canonicalize(root).unwrap());
    }

    #[test]
    fn from_nested_subdirectory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let nested = root.join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir(root.join(".devcontainer")).unwrap();

        let ws = find_workspace_from(&nested).unwrap();
        assert_eq!(ws.root, fs::canonicalize(root).unwrap());
    }

    #[test]
    fn from_inside_devcontainer() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let devcontainer = root.join(".devcontainer");
        fs::create_dir(&devcontainer).unwrap();

        let ws = find_workspace_from(&devcontainer).unwrap();
        assert_eq!(ws.root, fs::canonicalize(root).unwrap());
    }

    #[test]
    fn no_devcontainer_anywhere() {
        let tmp = TempDir::new().unwrap();
        let deep = tmp.path().join("x/y/z");
        fs::create_dir_all(&deep).unwrap();

        let err = find_workspace_from(&deep).unwrap_err();
        assert!(
            err.to_string().contains("devcontainer"),
            "expected error to mention 'devcontainer', got: {err}"
        );
    }

    #[test]
    fn root_is_workspace() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join(".devcontainer")).unwrap();

        let ws = find_workspace_from(root).unwrap();
        assert_eq!(ws.root, fs::canonicalize(root).unwrap());
    }
}
