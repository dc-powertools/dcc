//! Absence-only state initialization and image-owned directory links.
//!
//! Existing durable trees are never traversed, reconciled or hashed. Exclusion
//! migration uses same-filesystem renames, retaining every displaced entry.
use std::path::{Path, PathBuf};

use anyhow::Context as _;

use crate::{
    cache::CacheDir,
    config::{StateEntry, StateKind},
    docker,
};

/// Serialize initialization and link migration across independent dcc processes.
/// The lock lives outside mounted state and the kernel releases it on process exit.
async fn lock(cache: &CacheDir) -> anyhow::Result<std::fs::File> {
    let path = cache
        .host_path
        .parent()
        .context("cache has no managed root")?
        .join(format!("{}.persistence.lock", cache.profile_name()));
    tokio::task::spawn_blocking(move || {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("failed to open persistence lock `{}`", path.display()))?;
        file.lock()
            .with_context(|| format!("failed to lock `{}`", path.display()))?;
        Ok(file)
    })
    .await
    .context("persistence lock worker failed")?
}

/// Initialize only absent roots, then prepare links through attached state mounts.
/// The preparation container uses this same image and user, without user hooks;
/// targets are verified in its image layer, never copied into durable storage.
pub(crate) async fn prepare(
    cache: &CacheDir,
    image: &str,
    state: &[StateEntry],
    user: &str,
    mount_destinations: &[String],
) -> anyhow::Result<()> {
    if state.is_empty() {
        return Ok(());
    }
    let boundary = cache
        .host_path
        .parent()
        .context("cache has no managed root")?;
    check_host_path(boundary, &cache.host_path)?;
    let _lock = lock(cache).await?;
    let state_root = cache.host_path.join("state");
    check_host_path(boundary, &state_root)?;
    std::fs::create_dir_all(&state_root)
        .with_context(|| format!("failed to create state root `{}`", state_root.display()))?;
    let mut missing = Vec::new();
    for entry in state {
        let host = host_path(cache, &entry.path);
        check_host_path(boundary, &host)?;
        match std::fs::symlink_metadata(&host) {
            Ok(metadata) => anyhow::ensure!(
                match entry.kind {
                    StateKind::Directory => metadata.is_dir(),
                    StateKind::File => metadata.is_file(),
                },
                "state path `{}` has incompatible host type at `{}`",
                entry.path,
                host.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(entry.clone())
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to inspect `{}`", host.display()))
            }
        }
    }
    // Always validate image roots, including existing state: Docker otherwise
    // follows an image-side symlink when attaching a mount to it.
    let mut script = GUARDS.to_string();
    for destination in mount_destinations {
        script.push_str(&format!("check_path {}\n", quote(destination)));
    }
    script.push_str(&initialization_script(state, &missing, user));
    let args = vec![
        "--rm".into(),
        "-u".into(),
        "root".into(),
        "--mount".into(),
        format!("type=bind,src={},dst=/dcc-initialize", state_root.display()),
        "--entrypoint".into(),
        "sh".into(),
        image.into(),
        "-c".into(),
        script,
    ];
    docker::run_to_completion(&args)
        .await
        .context("failed to initialize absent state roots or verify image layout")?;
    let mounts = cache.plan_state_mounts(state);
    cache.prepare_state_mounts(&mounts)?;
    if state.iter().all(|entry| entry.exclusions.is_empty()) {
        return Ok(());
    }
    let mut args = vec!["--rm".into(), "-u".into(), user.into()];
    for mount in mounts {
        args.extend(["--mount".into(), mount.to_mount_arg()]);
    }
    // Recheck through attached durable roots too: user data can contain a
    // mount-destination alias absent from the image.
    let mut script = GUARDS.to_string();
    for destination in mount_destinations {
        script.push_str(&format!("check_path {}\n", quote(destination)));
    }
    script.push_str(&link_script(state));
    args.extend([
        "--entrypoint".into(),
        "sh".into(),
        image.into(),
        "-c".into(),
        script,
    ]);
    docker::run_to_completion(&args).await.context("failed to prepare image-owned state exclusions (displaced paths remain in .dcc-exclusion-backup.* directories)")
}

fn host_path(cache: &CacheDir, container: &str) -> PathBuf {
    cache
        .host_path
        .join("state")
        .join(container.trim_start_matches('/'))
}

/// Inspect only path components; never descend into existing durable data.
fn check_host_path(boundary: &Path, path: &Path) -> anyhow::Result<()> {
    let mut current = boundary.to_path_buf();
    let relative = path
        .strip_prefix(boundary)
        .context("state escaped cache root")?;
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(component) = component {
            current.push(component);
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => anyhow::ensure!(
                !metadata.file_type().is_symlink(),
                "refusing symlinked state ancestor `{}`",
                current.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to inspect state ancestor `{}`", current.display())
                })
            }
        }
    }
    Ok(())
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

const GUARDS: &str = r#"set -eu
fail() { echo "dcc persistence: $*" >&2; exit 1; }
check_path() {
    rest=${1#/}; current=
    while [ -n "$rest" ]; do
        part=${rest%%/*}; current=$current/$part
        [ ! -L "$current" ] || fail "symlinked ancestor: $current"
        if [ "$rest" = "$part" ]; then break; fi
        rest=${rest#*/}
    done
}
"#;

fn initialization_script(all: &[StateEntry], missing: &[StateEntry], user: &str) -> String {
    let mut script = GUARDS.to_string();
    for entry in all {
        script.push_str(&format!("check_path {}\n", quote(&entry.path)));
        // The installer owns relocation; generic dcc never moves image bytes.
        for excluded in &entry.exclusions {
            let original = format!("{}/{}", entry.path, excluded.path);
            script.push_str(&format!(
                "check_path {target}\n[ -d {target} ] || fail 'missing excluded image directory'\nif [ -e {original} ] || [ -L {original} ]; then [ -L {original} ] && [ \"$(readlink {original})\" = {target} ] || fail 'installer must relocate excluded directory and create its exact absolute symlink'; fi\n",
                target=quote(&excluded.target), original=quote(&original)));
        }
    }
    for entry in missing {
        let source = quote(&entry.path);
        let destination = format!("/dcc-initialize{}", entry.path);
        let parent = Path::new(&destination)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/dcc-initialize".into());
        script.push_str(&format!(
            "check_path {}\nmkdir -p {}\n",
            quote(&parent),
            quote(&parent)
        ));
        // A sibling staging directory is renamed only after a complete copy.
        // Failed staging remains recoverable, and existing destinations fail closed.
        script.push_str(&format!("[ ! -e {dst} ] && [ ! -L {dst} ] || fail 'state appeared during initialization'\nstage=$(mktemp -d {template})\n", dst=quote(&destination), template=quote(&format!("{parent}/.dcc-initialize.XXXXXX"))));
        match entry.kind {
            StateKind::Directory => script.push_str(&format!("if [ -e {src} ]; then [ -d {src} ] || fail 'image state is not a directory'; cp -a {src}/. \"$stage/root\"; else mkdir \"$stage/root\"; fi\n",src=source)),
            StateKind::File => script.push_str(&format!("if [ -e {src} ]; then [ -f {src} ] || fail 'image state is not a file'; cp -p {src} \"$stage/root\"; else : > \"$stage/root\"; fi\n",src=source)),
        }
        script.push_str(&format!(
            "chown -hR {} \"$stage/root\"\nmv -T -n \"$stage/root\" {}\n[ ! -e \"$stage/root\" ] && [ ! -L \"$stage/root\" ] || fail 'state appeared during initialization; staging preserved'\nrmdir \"$stage\"\n",
            quote(user),
            quote(&destination)
        ));
    }
    script
}

/// Run as the configured container user. No recursive walk, copy or chown.
fn link_script(state: &[StateEntry]) -> String {
    let mut script = GUARDS.to_string();
    for entry in state {
        for excluded in &entry.exclusions {
            let original = format!("{}/{}", entry.path, excluded.path);
            let parent = Path::new(&original)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| entry.path.clone());
            script.push_str(&format!(
                "check_path {root}\ncheck_path {target}\n[ -d {target} ] && [ -w {target} ] && [ -x {target} ] || fail 'excluded target must be a writable image directory'\ncheck_path {parent}\nmkdir -p {parent}\nif [ -L {original} ] && [ \"$(readlink {original})\" = {target} ]; then :; else\n  if [ -e {original} ] || [ -L {original} ]; then\n    backup=$(mktemp -d {backup})\n    printf '%s\n' {original} > \"$backup/original-path\"\n    mv {original} \"$backup/original\"\n    echo \"dcc: preserved displaced exclusion at $backup/original\" >&2\n  fi\n  ln -s {target} {original}\nfi\n",
                root=quote(&entry.path),target=quote(&excluded.target),parent=quote(&parent),original=quote(&original),backup=quote(&format!("{}/.dcc-exclusion-backup.XXXXXX",entry.path))));
        }
    }
    script
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::{symlink, PermissionsExt as _};
    use std::{
        fs,
        process::{Command, Output},
    };

    fn entry(root: &Path, child: &str, target: &Path) -> StateEntry {
        StateEntry {
            path: root.to_str().unwrap().into(),
            kind: StateKind::Directory,
            exclusions: vec![crate::config::StateExclusion {
                path: child.into(),
                target: target.to_str().unwrap().into(),
            }],
        }
    }

    fn run(script: String) -> Output {
        Command::new("sh").arg("-c").arg(script).output().unwrap()
    }

    fn assert_ok(output: Output) {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn backups(root: &Path) -> Vec<PathBuf> {
        fs::read_dir(root)
            .unwrap()
            .map(|item| item.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".dcc-exclusion-backup.")
            })
            .collect()
    }

    #[test]
    fn existing_host_tree_is_not_walked() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("state/root");
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/data"), "preserve").unwrap();
        check_host_path(tmp.path(), &root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_broken_host_symlink_without_following_it() {
        let tmp = tempfile::tempdir().unwrap();
        symlink("/missing", tmp.path().join("state")).unwrap();
        assert!(check_host_path(tmp.path(), &tmp.path().join("state/root")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn shell_migration_preserves_real_tree_and_is_idempotent_with_quoted_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("state ' quoted");
        let target = tmp.path().join("image ' quoted");
        fs::create_dir_all(root.join("packages/deep")).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(root.join("packages/deep/old-install"), "recover-me").unwrap();
        fs::write(root.join("auth"), "credential").unwrap();
        fs::write(target.join("release"), "image-install").unwrap();
        let state = vec![entry(&root, "packages", &target)];
        assert_ok(run(link_script(&state)));
        assert_eq!(fs::read_link(root.join("packages")).unwrap(), target);
        let saved = backups(&root);
        assert_eq!(saved.len(), 1);
        assert_eq!(
            fs::read(saved[0].join("original/deep/old-install")).unwrap(),
            b"recover-me"
        );
        assert_eq!(
            fs::read_to_string(saved[0].join("original-path")).unwrap(),
            format!("{}/packages\n", root.display())
        );
        assert_eq!(fs::read(root.join("auth")).unwrap(), b"credential");
        assert_ok(run(link_script(&state)));
        assert_eq!(backups(&root), saved);
        assert_eq!(fs::read(target.join("release")).unwrap(), b"image-install");
    }

    #[cfg(unix)]
    #[test]
    fn shell_migration_preserves_dangling_and_resolving_stale_links() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("state");
        let target = tmp.path().join("image");
        let outside = tmp.path().join("user-data");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&target).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("credential"), "keep").unwrap();
        let state = vec![entry(&root, "packages", &target)];
        symlink(tmp.path().join("missing"), root.join("packages")).unwrap();
        assert_ok(run(link_script(&state)));
        fs::remove_file(root.join("packages")).unwrap();
        symlink(&outside, root.join("packages")).unwrap();
        assert_ok(run(link_script(&state)));
        let saved = backups(&root);
        assert_eq!(saved.len(), 2);
        assert!(saved
            .iter()
            .any(|path| fs::read_link(path.join("original")).unwrap() == outside));
        assert_eq!(fs::read(outside.join("credential")).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    }

    #[test]
    fn missing_image_target_fails_before_migrating_durable_content() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("state");
        fs::create_dir_all(root.join("packages")).unwrap();
        fs::write(root.join("packages/credential"), "keep").unwrap();
        let state = vec![entry(&root, "packages", &tmp.path().join("missing"))];
        assert!(!run(initialization_script(&state, &[], "root"))
            .status
            .success());
        assert!(!run(link_script(&state)).status.success());
        assert_eq!(fs::read(root.join("packages/credential")).unwrap(), b"keep");
        assert!(backups(&root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn image_target_symlink_ancestor_is_refused_before_durable_migration() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("state");
        let image = tmp.path().join("image");
        fs::create_dir_all(root.join("packages")).unwrap();
        fs::create_dir_all(image.join("packages")).unwrap();
        fs::write(root.join("packages/credential"), "keep").unwrap();
        symlink(&image, tmp.path().join("alias")).unwrap();
        let state = vec![entry(&root, "packages", &tmp.path().join("alias/packages"))];
        assert!(!run(initialization_script(&state, &[], "root"))
            .status
            .success());
        assert!(!run(link_script(&state)).status.success());
        assert_eq!(fs::read(root.join("packages/credential")).unwrap(), b"keep");
        assert!(backups(&root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn nested_exclusion_symlink_parent_does_not_touch_user_target() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("state");
        let target = tmp.path().join("image");
        let outside = tmp.path().join("user-data");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&target).unwrap();
        fs::create_dir_all(outside.join("packages")).unwrap();
        fs::write(outside.join("packages/credential"), "keep").unwrap();
        symlink(&outside, root.join("nested")).unwrap();
        let state = vec![entry(&root, "nested/packages", &target)];
        assert!(!run(link_script(&state)).status.success());
        assert_eq!(fs::read_link(root.join("nested")).unwrap(), outside);
        assert_eq!(
            fs::read(outside.join("packages/credential")).unwrap(),
            b"keep"
        );
        assert!(backups(&root).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn existing_state_initialization_never_requires_recursive_ownership_changes() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("state");
        fs::create_dir_all(root.join("private")).unwrap();
        fs::write(root.join("private/credential"), "keep").unwrap();
        let original = fs::metadata(root.join("private/credential")).unwrap();
        fs::set_permissions(root.join("private"), fs::Permissions::from_mode(0o0)).unwrap();
        let state = vec![StateEntry {
            path: root.to_str().unwrap().into(),
            kind: StateKind::Directory,
            exclusions: vec![],
        }];
        // Any accidental chown of existing state fails for this invalid user.
        let output = run(initialization_script(
            &state,
            &[],
            "dcc-user-that-does-not-exist",
        ));
        fs::set_permissions(root.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
        assert_ok(output);
        assert_eq!(fs::read(root.join("private/credential")).unwrap(), b"keep");
        assert_eq!(
            fs::metadata(root.join("private/credential"))
                .unwrap()
                .permissions(),
            original.permissions()
        );
    }
}
