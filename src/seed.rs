//! Seeding declared `customizations.dcc.state` from the image.
//!
//! Bind mounts mask image content with an empty host source, so data a Feature
//! `install.sh`, a `Dockerfile` layer, or an official `build` source placed at a
//! declared state path is invisible at runtime. This module hydrates the
//! profile-local host state directory by copying image content out of a
//! short-lived container that runs **without** the state mounts applied, then
//! records what it wrote in a host-side ledger at `.dcc/<profile>.seed.json`.
//!
//! See `.meta/decisions/0001-state-seeding-from-image.md` for the design and
//! the rejected alternatives (a baked seed store and `mv`-based relocation).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::{
    cache::CacheDir,
    config::{StateEntry, StateKind},
    docker,
};

/// The container path at which the host state root is mounted inside the
/// hydration container. Chosen to be unrelated to any real state path so both
/// sides are visible simultaneously without masking.
pub(crate) const SEED_MOUNT_DST: &str = "/dcc-seed";

/// One entry in the `dcc.seed` image label: a resolved state path, its kind, and
/// a content digest (or an explicit absence marker when no digest could be
/// computed). The label is what lets `--dry-run` report planned seeding without
/// invoking Docker.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct SeedManifestEntry {
    pub(crate) path: String,
    #[serde(rename = "type", default = "directory_lowercase")]
    pub(crate) kind: SeedKind,
    /// SHA-256 hex digest of the seeded content, or `None` when no digest could
    /// be computed. Serialized as an explicit `null` so an absent digest never
    /// collides with a real one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) digest: Option<String>,
}

/// Manifest kind, serialized as lowercase `directory`/`file` to match
/// `StateKind`'s serde representation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SeedKind {
    Directory,
    File,
}

impl From<StateKind> for SeedKind {
    fn from(kind: StateKind) -> Self {
        match kind {
            StateKind::Directory => SeedKind::Directory,
            StateKind::File => SeedKind::File,
        }
    }
}

fn directory_lowercase() -> SeedKind {
    SeedKind::Directory
}

/// The full `dcc.seed` label value: a build id plus one entry per declared
/// state path, in declaration order.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct SeedManifest {
    pub(crate) build_id: String,
    pub(crate) entries: Vec<SeedManifestEntry>,
}

impl SeedManifest {
    pub(crate) fn empty(build_id: String) -> Self {
        Self {
            build_id,
            entries: Vec::new(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Serializes the manifest for the `dcc.seed` image label.
    pub(crate) fn to_label(&self) -> anyhow::Result<String> {
        serde_json::to_string(self).context("failed to serialize dcc.seed label")
    }

    /// Parses a `dcc.seed` label value.
    pub(crate) fn from_label(json: &str) -> anyhow::Result<Self> {
        serde_json::from_str(json)
            .with_context(|| "failed to parse dcc.seed label as JSON".to_string())
    }
}

/// One per-entry record in the host-side ledger at `.dcc/<profile>.seed.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub(crate) struct LedgerEntry {
    pub(crate) path: String,
    #[serde(rename = "type", default = "directory_lowercase")]
    pub(crate) kind: SeedKind,
    /// Digest of the content `dcc` wrote at seed time. `None` means `dcc`
    /// seeded the entry but could not compute a digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) seed_digest: Option<String>,
    /// Image identity that produced this seed.
    pub(crate) build_id: String,
}

/// The host-side ledger, authoritative for hydration decisions. Lives outside
/// the `/cache` mount so container-side code cannot reach it.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub(crate) struct SeedLedger {
    pub(crate) entries: Vec<LedgerEntry>,
}

impl SeedLedger {
    pub(crate) fn path(cache_dir: &CacheDir) -> PathBuf {
        // Sibling of the profile cache directory: <workspace>/.dcc/<profile>.seed.json
        cache_dir
            .host_path
            .parent()
            .unwrap_or_else(|| Path::new("/"))
            .join(format!("{}.seed.json", cache_dir.profile_name()))
    }

    pub(crate) fn read(path: &Path) -> anyhow::Result<Self> {
        match std::fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .with_context(|| format!("failed to parse seed ledger `{}`", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => {
                Err(e).with_context(|| format!("failed to read seed ledger `{}`", path.display()))
            }
        }
    }

    pub(crate) fn write(&self, path: &Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self).context("failed to serialize seed ledger")?;
        std::fs::write(path, json)
            .with_context(|| format!("failed to write seed ledger `{}`", path.display()))
    }

    /// Returns the ledger record for `path`, if any.
    pub(crate) fn get(&self, path: &str) -> Option<&LedgerEntry> {
        self.entries.iter().find(|e| e.path == path)
    }
}

/// What `dcc build` decided to do with one state entry: seed it fresh, refresh
/// it (digest matched so overwriting is safe), skip it (ledger says seeded and
/// nothing changed), or preserve user data (digest differs, no `--reseed-state`).
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum HydrationDecision {
    /// No ledger record; seed from the image.
    Seed { path: String, kind: SeedKind },
    /// Ledger digest matches the current host state; safe to overwrite silently.
    Refresh { path: String, kind: SeedKind },
    /// Ledger digest differs from the host state; user has real data, do not clobber.
    Preserve { path: String, kind: SeedKind },
    /// Ledger present and digest matches; nothing to do.
    Skip { path: String, kind: SeedKind },
}

/// Computes the per-entry hydration plan for `dcc build`, given the seed manifest
/// from the image label, the current ledger, and the host state digests.
///
/// `host_digests` maps each resolved state path to a digest of its current host
/// content (or `None` when the host path is empty/absent). With
/// `reseed_state = true`, differing host state is overwritten (`Refresh`)
/// instead of preserved.
pub(crate) fn plan_hydration(
    manifest: &SeedManifest,
    ledger: &SeedLedger,
    host_digests: &HashMap<String, Option<String>>,
    reseed_state: bool,
) -> Vec<HydrationDecision> {
    manifest
        .entries
        .iter()
        .map(|entry| {
            let ledger_entry = ledger.get(&entry.path);
            let host_digest = host_digests.get(&entry.path).cloned().flatten();
            match (ledger_entry, host_digest) {
                (None, _) => HydrationDecision::Seed {
                    path: entry.path.clone(),
                    kind: entry.kind,
                },
                (Some(rec), Some(ref hd)) if rec.seed_digest.as_ref() == Some(hd) => {
                    // Host state matches what dcc last wrote: already seeded, no
                    // work needed beyond the digest check.
                    HydrationDecision::Skip {
                        path: entry.path.clone(),
                        kind: entry.kind,
                    }
                }
                (Some(_rec), Some(_)) => {
                    // Host state differs from the recorded seed digest.
                    if reseed_state {
                        HydrationDecision::Refresh {
                            path: entry.path.clone(),
                            kind: entry.kind,
                        }
                    } else {
                        HydrationDecision::Preserve {
                            path: entry.path.clone(),
                            kind: entry.kind,
                        }
                    }
                }
                (Some(_rec), None) => {
                    // Host path empty/absent but ledger says seeded: re-seed.
                    HydrationDecision::Seed {
                        path: entry.path.clone(),
                        kind: entry.kind,
                    }
                }
            }
        })
        .collect()
}

/// The subset of `HydrationDecision` that requires actual Docker hydration.
pub(crate) fn needs_hydration(plan: &[HydrationDecision]) -> bool {
    plan.iter().any(|d| {
        matches!(
            d,
            HydrationDecision::Seed { .. } | HydrationDecision::Refresh { .. }
        )
    })
}

/// Builds the argument list for the one-shot hydration container:
///
/// ```text
/// docker run --rm -u root \
///   --mount type=bind,src=<state-root>,dst=/dcc-seed \
///   <image> sh -c '<per-entry tar copy>'
/// ```
///
/// State mounts are deliberately NOT applied, so the image content at each
/// declared path is visible. The copy runs inside the container to preserve
/// uid, gid, mode, and symlinks.
pub(crate) fn hydration_container_args(
    image: &str,
    state_root: &str,
    entries: &[SeedManifestEntry],
) -> Vec<String> {
    let mut args = vec![
        "--rm".to_string(),
        "-u".to_string(),
        "root".to_string(),
        "--mount".to_string(),
        format!("type=bind,src={state_root},dst={SEED_MOUNT_DST}"),
        image.to_string(),
        "sh".to_string(),
        "-c".to_string(),
        hydration_copy_script(entries),
    ];
    // Ensure the top-level /dcc-seed exists even when there are no entries.
    let _ = &mut args;
    args
}

/// Renders the shell copy script run inside the hydration container. Each entry
/// is copied with `tar` so ownership, mode, and symlinks are preserved. The
/// destination is `/dcc-seed/<normalized>`, where `<normalized>` is the
/// container path with the leading `/` stripped.
fn hydration_copy_script(entries: &[SeedManifestEntry]) -> String {
    let mut lines = vec!["set -eu".to_string()];
    for entry in entries {
        let container_path = &entry.path;
        let rel = container_path.trim_start_matches('/');
        let dst_dir = format!("{SEED_MOUNT_DST}/{rel}");
        // For directory state, copy the whole tree. For file state, copy the
        // single file (preserving its parent layout under /dcc-seed).
        match entry.kind {
            SeedKind::Directory => {
                lines.push(format!(
                    "if [ -e '{container_path}' ]; then mkdir -p '{dst_dir}' && \
                     tar -C / -cf - -- '{rel}' | tar -C '{SEED_MOUNT_DST}' -xf -; fi"
                ));
            }
            SeedKind::File => {
                lines.push(format!(
                    "if [ -e '{container_path}' ]; then mkdir -p '{dst_dir}' && \
                     tar -C / -cf - -- '{rel}' | tar -C '{SEED_MOUNT_DST}' -xf -; fi"
                ));
            }
        }
    }
    lines.join("; ")
}

/// Computes a SHA-256 hex digest of a host path's content. Directories are
/// digested over a sorted, deterministic tar of their contents; files over
/// their raw bytes. Returns `None` when the path does not exist or is empty
/// (no files / zero-length file), so an empty seeded directory and an unseeded
/// one remain distinguishable via the ledger.
pub(crate) fn host_state_digest(host_path: &Path) -> anyhow::Result<Option<String>> {
    if !host_path.exists() {
        return Ok(None);
    }
    if host_path.is_file() {
        let bytes = std::fs::read(host_path)
            .with_context(|| format!("failed to read state file `{}`", host_path.display()))?;
        if bytes.is_empty() {
            return Ok(None);
        }
        return Ok(Some(sha256_hex(&bytes)));
    }
    if host_path.is_dir() {
        let digest = digest_directory(host_path)?;
        return Ok(digest);
    }
    Ok(None)
}

/// Digests a directory by streaming a deterministic tar (sorted entries) into
/// SHA-256. Returns `None` when the directory contains no regular files.
fn digest_directory(dir: &Path) -> anyhow::Result<Option<String>> {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    let mut entries = collect_entries(dir)?;
    if entries.is_empty() {
        return Ok(None);
    }
    entries.sort();
    let mut tar = tar::Builder::new(Vec::new());
    for rel in &entries {
        let abs = dir.join(rel);
        let metadata = std::fs::symlink_metadata(&abs)
            .with_context(|| format!("failed to stat `{}`", abs.display()))?;
        let mut header = tar::Header::new_gnu();
        header.set_size(metadata.len());
        header.set_mode(0o644);
        header.set_mtime(0);
        if metadata.is_dir() {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
        } else if metadata.is_symlink() {
            let target = std::fs::read_link(&abs)
                .with_context(|| format!("failed to read symlink `{}`", abs.display()))?;
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name(target.to_string_lossy().as_ref())?;
            header.set_size(0);
        } else if metadata.is_file() {
            header.set_entry_type(tar::EntryType::Regular);
        } else {
            continue;
        };
        header.set_cksum();
        tar.append_data(&mut header, rel, &mut std::io::empty())
            .with_context(|| format!("failed to append tar header for `{rel}`"))?;
        if metadata.is_file() {
            let mut f = std::fs::File::open(&abs)
                .with_context(|| format!("failed to open `{}`", abs.display()))?;
            tar.append(&header, &mut f)
                .with_context(|| format!("failed to append file `{rel}` to digest tar"))?;
        }
    }
    let tar_bytes = tar.into_inner().context("failed to finalize digest tar")?;
    hasher.update(&tar_bytes);
    Ok(Some(hex::encode(&hasher.finalize())))
}

fn collect_entries(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    walk(dir, "", &mut out)?;
    Ok(out)
}

fn walk(base: &Path, rel: &str, out: &mut Vec<String>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(base)
        .with_context(|| format!("failed to read directory `{}`", base.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let child_rel = if rel.is_empty() {
            name.to_string()
        } else {
            format!("{rel}/{name}")
        };
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            out.push(format!("{child_rel}/"));
            walk(&entry.path(), &child_rel, out)?;
        } else {
            out.push(child_rel);
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hex::encode(&hasher.finalize())
}

mod hex {
    pub(crate) fn encode(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

/// Reads the `dcc.seed` image label for `image`, returning an empty manifest
/// (with the given build id) when the label is absent.
pub(crate) async fn read_manifest_from_image(
    image: &str,
    build_id: &str,
) -> anyhow::Result<SeedManifest> {
    match docker::inspect_image_label_value(image, "dcc.seed").await? {
        None => Ok(SeedManifest::empty(build_id.to_string())),
        Some(json) => SeedManifest::from_label(&json),
    }
}

/// Returns the host state root path for a profile: `<workspace>/.dcc/<profile>/state`.
pub(crate) fn state_root(cache_dir: &CacheDir) -> PathBuf {
    cache_dir.host_path.join("state")
}

/// Returns the host path for a single resolved state entry.
pub(crate) fn state_host_path(cache_dir: &CacheDir, container_path: &str) -> PathBuf {
    let mut host_path = state_root(cache_dir);
    for segment in container_path.trim_start_matches('/').split('/') {
        if !segment.is_empty() {
            host_path.push(segment);
        }
    }
    host_path
}

/// Builds a seed manifest from resolved state entries and a build id. Digests
/// are left `None` here; they are filled in after hydration (or read from the
/// label at runtime).
pub(crate) fn manifest_from_state(state: &[StateEntry], build_id: &str) -> SeedManifest {
    SeedManifest {
        build_id: build_id.to_string(),
        entries: state
            .iter()
            .map(|entry| SeedManifestEntry {
                path: entry.path.clone(),
                kind: entry.kind.into(),
                digest: None,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(path: &str) -> StateEntry {
        StateEntry {
            path: path.to_string(),
            kind: StateKind::Directory,
        }
    }

    fn file(path: &str) -> StateEntry {
        StateEntry {
            path: path.to_string(),
            kind: StateKind::File,
        }
    }

    fn manifest(entries: &[SeedManifestEntry]) -> SeedManifest {
        SeedManifest {
            build_id: "img-1".to_string(),
            entries: entries.to_vec(),
        }
    }

    fn ledger_entry(path: &str, kind: SeedKind, digest: Option<&str>) -> LedgerEntry {
        LedgerEntry {
            path: path.to_string(),
            kind,
            seed_digest: digest.map(str::to_string),
            build_id: "img-1".to_string(),
        }
    }

    fn host_digests(pairs: &[(&str, Option<&str>)]) -> HashMap<String, Option<String>> {
        pairs
            .iter()
            .map(|(p, d)| (p.to_string(), d.map(str::to_string)))
            .collect()
    }

    // ── manifest / label round-trip ───────────────────────────────────────────

    #[test]
    fn manifest_round_trips_through_label() {
        let m = manifest(&[
            SeedManifestEntry {
                path: "/home/dev/.cargo".to_string(),
                kind: SeedKind::Directory,
                digest: Some("abc".to_string()),
            },
            SeedManifestEntry {
                path: "/home/dev/.npmrc".to_string(),
                kind: SeedKind::File,
                digest: None,
            },
        ]);
        let label = m.to_label().unwrap();
        let parsed = SeedManifest::from_label(&label).unwrap();
        assert_eq!(parsed, m);
    }

    #[test]
    fn empty_manifest_is_empty() {
        assert!(SeedManifest::empty("img".to_string()).is_empty());
    }

    #[test]
    fn manifest_kind_serializes_lowercase() {
        let m = manifest(&[SeedManifestEntry {
            path: "/x".to_string(),
            kind: SeedKind::File,
            digest: None,
        }]);
        let label = m.to_label().unwrap();
        assert!(label.contains(r#""type":"file""#), "got: {label}");
    }

    // ── ledger read/write round-trip ──────────────────────────────────────────

    #[test]
    fn ledger_round_trips_through_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("dev.seed.json");
        let ledger = SeedLedger {
            entries: vec![ledger_entry("/x", SeedKind::Directory, Some("deadbeef"))],
        };
        ledger.write(&path).unwrap();
        let read = SeedLedger::read(&path).unwrap();
        assert_eq!(read, ledger);
    }

    #[test]
    fn ledger_read_missing_file_is_default() {
        let read = SeedLedger::read(Path::new("/nonexistent/ledger.json")).unwrap();
        assert!(read.entries.is_empty());
    }

    #[test]
    fn ledger_get_finds_entry() {
        let ledger = SeedLedger {
            entries: vec![ledger_entry("/x", SeedKind::Directory, Some("d"))],
        };
        assert!(ledger.get("/x").is_some());
        assert!(ledger.get("/y").is_none());
    }

    // ── plan_hydration decision branches ──────────────────────────────────────

    #[test]
    fn plan_seeds_when_no_ledger_record() {
        let m = manifest(&[SeedManifestEntry {
            path: "/x".to_string(),
            kind: SeedKind::Directory,
            digest: None,
        }]);
        let plan = plan_hydration(
            &m,
            &SeedLedger::default(),
            &host_digests(&[("/x", None)]),
            false,
        );
        assert_eq!(
            plan,
            vec![HydrationDecision::Seed {
                path: "/x".to_string(),
                kind: SeedKind::Directory
            }]
        );
    }

    #[test]
    fn plan_skips_when_digest_matches_ledger() {
        let m = manifest(&[SeedManifestEntry {
            path: "/x".to_string(),
            kind: SeedKind::Directory,
            digest: None,
        }]);
        let ledger = SeedLedger {
            entries: vec![ledger_entry("/x", SeedKind::Directory, Some("abc"))],
        };
        let plan = plan_hydration(&m, &ledger, &host_digests(&[("/x", Some("abc"))]), false);
        assert_eq!(
            plan,
            vec![HydrationDecision::Skip {
                path: "/x".to_string(),
                kind: SeedKind::Directory
            }]
        );
    }

    #[test]
    fn plan_preserves_when_digest_differs_without_reseed() {
        let m = manifest(&[SeedManifestEntry {
            path: "/x".to_string(),
            kind: SeedKind::Directory,
            digest: None,
        }]);
        let ledger = SeedLedger {
            entries: vec![ledger_entry("/x", SeedKind::Directory, Some("abc"))],
        };
        let plan = plan_hydration(
            &m,
            &ledger,
            &host_digests(&[("/x", Some("user-data"))]),
            false,
        );
        assert_eq!(
            plan,
            vec![HydrationDecision::Preserve {
                path: "/x".to_string(),
                kind: SeedKind::Directory
            }]
        );
    }

    #[test]
    fn plan_refreshes_when_digest_differs_with_reseed() {
        let m = manifest(&[SeedManifestEntry {
            path: "/x".to_string(),
            kind: SeedKind::Directory,
            digest: None,
        }]);
        let ledger = SeedLedger {
            entries: vec![ledger_entry("/x", SeedKind::Directory, Some("abc"))],
        };
        let plan = plan_hydration(
            &m,
            &ledger,
            &host_digests(&[("/x", Some("user-data"))]),
            true,
        );
        assert_eq!(
            plan,
            vec![HydrationDecision::Refresh {
                path: "/x".to_string(),
                kind: SeedKind::Directory
            }]
        );
    }

    #[test]
    fn plan_seeds_when_ledger_present_but_host_empty() {
        let m = manifest(&[SeedManifestEntry {
            path: "/x".to_string(),
            kind: SeedKind::Directory,
            digest: None,
        }]);
        let ledger = SeedLedger {
            entries: vec![ledger_entry("/x", SeedKind::Directory, Some("abc"))],
        };
        // Host path absent → digest None.
        let plan = plan_hydration(&m, &ledger, &host_digests(&[("/x", None)]), false);
        assert_eq!(
            plan,
            vec![HydrationDecision::Seed {
                path: "/x".to_string(),
                kind: SeedKind::Directory
            }]
        );
    }

    #[test]
    fn needs_hydration_true_for_seed_and_refresh() {
        let plan = vec![
            HydrationDecision::Seed {
                path: "/a".to_string(),
                kind: SeedKind::Directory,
            },
            HydrationDecision::Preserve {
                path: "/b".to_string(),
                kind: SeedKind::Directory,
            },
        ];
        assert!(needs_hydration(&plan));
        let plan = vec![HydrationDecision::Preserve {
            path: "/b".to_string(),
            kind: SeedKind::Directory,
        }];
        assert!(!needs_hydration(&plan));
    }

    // ── hydration container args ──────────────────────────────────────────────

    #[test]
    fn hydration_container_args_mount_state_root_and_run_tar() {
        let entries = vec![
            SeedManifestEntry {
                path: "/home/dev/.cargo".to_string(),
                kind: SeedKind::Directory,
                digest: None,
            },
            SeedManifestEntry {
                path: "/home/dev/.npmrc".to_string(),
                kind: SeedKind::File,
                digest: None,
            },
        ];
        let args = hydration_container_args("img", "/ws/.dcc/dev/state", &entries);
        assert!(args.contains(&"--rm".to_string()));
        assert!(args.contains(&"-u".to_string()));
        assert!(args.contains(&"root".to_string()));
        assert!(args.contains(&"img".to_string()));
        assert!(
            args.iter()
                .any(|a| a == "type=bind,src=/ws/.dcc/dev/state,dst=/dcc-seed"),
            "expected seed bind mount, got: {args:?}"
        );
        let script = args.last().expect("script is last arg");
        assert!(script.contains("tar -C / -cf -"), "got: {script}");
        assert!(script.contains("home/dev/.cargo"), "got: {script}");
        assert!(script.contains("home/dev/.npmrc"), "got: {script}");
    }

    #[test]
    fn hydration_container_args_no_state_entries_is_just_set_eu() {
        let args = hydration_container_args("img", "/ws/.dcc/dev/state", &[]);
        let script = args.last().unwrap();
        assert_eq!(script, "set -eu");
    }

    // ── host_state_digest ─────────────────────────────────────────────────────

    #[test]
    fn host_digest_missing_path_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let d = host_state_digest(&tmp.path().join("nope")).unwrap();
        assert_eq!(d, None);
    }

    #[test]
    fn host_digest_empty_file_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("empty");
        std::fs::write(&p, "").unwrap();
        assert_eq!(host_state_digest(&p).unwrap(), None);
    }

    #[test]
    fn host_digest_nonempty_file_is_some_and_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("f");
        std::fs::write(&p, "hello").unwrap();
        let d1 = host_state_digest(&p).unwrap().unwrap();
        let d2 = host_state_digest(&p).unwrap().unwrap();
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }

    #[test]
    fn host_digest_empty_directory_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("d");
        std::fs::create_dir(&p).unwrap();
        assert_eq!(host_state_digest(&p).unwrap(), None);
    }

    #[test]
    fn host_digest_nonempty_directory_is_some_and_stable() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("d");
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("a"), "a").unwrap();
        std::fs::write(p.join("b"), "b").unwrap();
        let d1 = host_state_digest(&p).unwrap().unwrap();
        let d2 = host_state_digest(&p).unwrap().unwrap();
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 64);
    }

    // ── manifest_from_state ───────────────────────────────────────────────────

    #[test]
    fn manifest_from_state_preserves_order_and_kinds() {
        let m = manifest_from_state(&[dir("/x"), file("/y/.npmrc")], "img-9");
        assert_eq!(m.build_id, "img-9");
        assert_eq!(m.entries.len(), 2);
        assert_eq!(m.entries[0].path, "/x");
        assert_eq!(m.entries[0].kind, SeedKind::Directory);
        assert_eq!(m.entries[1].path, "/y/.npmrc");
        assert_eq!(m.entries[1].kind, SeedKind::File);
        assert!(m.entries.iter().all(|e| e.digest.is_none()));
    }

    // ── state_host_path ───────────────────────────────────────────────────────

    fn cache(root: &Path) -> CacheDir {
        CacheDir {
            host_path: root.join(".dcc/dev"),
            profile_name: crate::profile::ProfileName::new("dev"),
        }
    }

    #[test]
    fn state_host_path_strips_leading_slash() {
        let tmp = tempfile::tempdir().unwrap();
        let c = cache(tmp.path());
        let p = state_host_path(&c, "/home/dev/.cargo");
        assert_eq!(p, tmp.path().join(".dcc/dev/state/home/dev/.cargo"));
    }

    #[test]
    fn seed_ledger_path_is_sibling_of_profile_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let c = cache(tmp.path());
        let ledger = SeedLedger::path(&c);
        assert_eq!(ledger, tmp.path().join(".dcc/dev.seed.json"));
        assert!(
            !ledger.starts_with(&c.host_path),
            "ledger must be outside /cache mount"
        );
    }
}
