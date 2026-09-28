use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;

#[derive(Debug)]
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    /// Stable string identifying this repository, used to derive dcc container ids.
    /// For git repos with an `origin` remote, this is the remote URL (same on every
    /// machine that clones the repo). Falls back to the canonical workspace root path
    /// for non-git workspaces or repos without a configured origin.
    pub(crate) identity: String,
}

pub(crate) fn find_workspace() -> anyhow::Result<Workspace> {
    let start = std::env::current_dir().context("failed to determine current working directory")?;
    find_workspace_from(&start)
}

fn find_workspace_from(start: &Path) -> anyhow::Result<Workspace> {
    let path = canonical_start(start)?;
    if let Some(root) = existing_workspace_root(&path) {
        return Ok(workspace_at(root));
    }

    anyhow::bail!(
        "could not find `.devcontainer/` directory in `{}` or any of its ancestors",
        path.display()
    )
}

/// Resolve the destination without creating it; bootstrap alone may initialize
/// a workspace that does not yet contain `.devcontainer`.
pub(crate) fn find_bootstrap_workspace() -> anyhow::Result<Workspace> {
    let start = std::env::current_dir().context("failed to determine current working directory")?;
    let path = canonical_start(&start)?;
    let root = match existing_workspace_root(&path) {
        Some(root) => root,
        None => git_worktree_root(&path)?.unwrap_or(path),
    };
    Ok(workspace_at(root))
}

fn canonical_start(start: &Path) -> anyhow::Result<PathBuf> {
    let mut path = fs::canonicalize(start)
        .with_context(|| format!("failed to canonicalize path: {}", start.display()))?;

    if path.file_name() == Some(std::ffi::OsStr::new(".devcontainer")) {
        path = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("`.devcontainer` has no parent directory"))?
            .to_path_buf();
    }

    Ok(path)
}

fn existing_workspace_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".devcontainer").is_dir())
        .map(Path::to_path_buf)
}

fn workspace_at(root: PathBuf) -> Workspace {
    let identity = git_remote_url(&root).unwrap_or_else(|| root.to_string_lossy().into_owned());
    Workspace { root, identity }
}

/// Git is optional. A missing executable or a directory outside a Git worktree
/// leaves bootstrap to use its current directory.
fn git_worktree_root(start: &Path) -> anyhow::Result<Option<PathBuf>> {
    let output = match std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(start)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("failed to run git rev-parse --show-toplevel"),
    };
    if !output.status.success() {
        return Ok(None);
    }
    let root = String::from_utf8(output.stdout)
        .context("git rev-parse --show-toplevel returned a non-UTF-8 path")?;
    let root = Path::new(root.strip_suffix('\n').unwrap_or(&root));
    let root = fs::canonicalize(root).with_context(|| {
        format!(
            "failed to canonicalize Git worktree root: {}",
            root.display()
        )
    })?;
    Ok(Some(root))
}

/// Returns the `origin` remote URL for the git repo at `root`, or `None` if
/// `root` is not a git repo, has no `origin` remote, or `git` is not installed.
fn git_remote_url(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8(output.stdout).ok()?;
    let url = url.trim().to_owned();
    if url.is_empty() {
        None
    } else {
        Some(url)
    }
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
}
