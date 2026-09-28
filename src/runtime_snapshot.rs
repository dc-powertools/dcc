//! Frozen, container-side reuse data. Never deserialize this into a host launch plan.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read as _;
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::{config, forward::PortMapping, lifecycle::LifecycleCommand};

pub(crate) const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeSnapshot {
    pub(crate) protocol: u32,
    pub(crate) identity: String,
    pub(crate) token: String,
    pub(crate) image: String,
    pub(crate) fingerprint: Option<String>,
    pub(crate) user: String,
    pub(crate) workdir: String,
    pub(crate) local_workspace: String,
    pub(crate) local_cache: String,
    pub(crate) container_env: HashMap<String, String>,
    pub(crate) attach_hooks: Vec<LifecycleCommand>,
    pub(crate) scripts: HashMap<String, String>,
    pub(crate) feature_scripts: Vec<(String, IndexMap<String, String>)>,
    pub(crate) ports: Vec<PortMapping>,
    pub(crate) memory: String,
    pub(crate) cpus: String,
}

impl RuntimeSnapshot {
    pub(crate) fn decode(
        bytes: &[u8],
        identity: &str,
        image: &str,
        token: &str,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(bytes.len() <= MAX_BYTES, "runtime snapshot exceeds 16 MiB");
        // Don't include serde's input-bearing error text: a malformed value can be a secret.
        let snapshot: Self = serde_json::from_slice(bytes)
            .map_err(|_| anyhow::anyhow!("invalid runtime snapshot; stop, rebuild and recreate"))?;
        anyhow::ensure!(
            snapshot.protocol == 1
                && snapshot.identity == identity
                && snapshot.image == image
                && snapshot.token == token
                && !token.is_empty(),
            "runtime snapshot identity/protocol mismatch; stop, rebuild and recreate"
        );
        anyhow::ensure!(
            !snapshot.user.is_empty()
                && snapshot.user.len() <= 256
                && !snapshot.user.contains('\0')
                && snapshot.workdir.starts_with('/')
                && snapshot.workdir.len() <= 4096
                && !snapshot.workdir.contains('\0'),
            "invalid runtime snapshot execution context"
        );
        anyhow::ensure!(
            snapshot
                .fingerprint
                .as_ref()
                .is_none_or(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
            "invalid snapshot fingerprint"
        );
        let mut targets = HashSet::new();
        let mut proxies = HashSet::new();
        for p in &snapshot.ports {
            anyhow::ensure!(
                p.target > 0
                    && p.proxy >= 1024
                    && targets.insert(p.target)
                    && proxies.insert(p.proxy),
                "invalid runtime snapshot port plan"
            );
        }
        anyhow::ensure!(
            targets.is_disjoint(&proxies),
            "overlapping snapshot proxy and application ports"
        );
        Ok(snapshot)
    }

    pub(crate) fn explicit_args(&self, args: &[String]) -> anyhow::Result<Vec<String>> {
        args.iter()
            .map(|arg| {
                let value =
                    config::vars::apply_substitution(arg, &self.local_workspace, &self.local_cache);
                config::vars::resolve_container_env(&value, &self.container_env)
            })
            .collect()
    }
}

fn record(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

fn file_bytes(path: &Path, optional: bool) -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::metadata(path) {
        Ok(meta) => anyhow::ensure!(meta.is_file(), "fingerprint input is not a regular file"),
        Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("cannot inspect configuration fingerprint input"),
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if optional && e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("cannot read configuration fingerprint input"),
    };
    anyhow::ensure!(
        file.metadata()?.is_file(),
        "fingerprint input is not a regular file"
    );
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= MAX_BYTES, "fingerprint input exceeds 16 MiB");
    Ok(Some(bytes))
}

fn environment_names(value: &str, names: &mut BTreeSet<String>) {
    for part in value.split("${localEnv:").skip(1) {
        if let Some((body, _)) = part.split_once('}') {
            let name = body.split(':').next().unwrap_or_default();
            if !name.is_empty() {
                names.insert(name.to_owned());
            }
        }
    }
}

/// Host-derived paths only. The snapshot cannot direct the CLI to read host files.
pub(crate) fn fingerprint(
    path: &Path,
    workspace: &Path,
    cache: &Path,
    metadata: &str,
) -> anyhow::Result<String> {
    let mut visited = HashSet::new();
    let raw = config::resolve::load_raw(path, &mut visited, false)?;
    let mut hash = Sha256::new();
    record(&mut hash, b"dcc-config-v1");
    record(&mut hash, workspace.to_string_lossy().as_bytes());
    record(&mut hash, cache.to_string_lossy().as_bytes());
    let mut names = BTreeSet::new();
    environment_names(metadata, &mut names);
    let mut files: std::collections::BTreeMap<_, bool> =
        visited.into_iter().map(|p| (p, false)).collect();
    let base = path.parent().context("configuration has no parent")?;
    if let Some(build) = &raw.build {
        let dockerfile = base.join(&build.dockerfile);
        files.insert(
            dockerfile.with_file_name(format!(
                "{}.dockerignore",
                dockerfile.file_name().unwrap_or_default().to_string_lossy()
            )),
            true,
        );
        files.insert(dockerfile, false);
        files.insert(base.join(&build.context).join(".dockerignore"), true);
    }
    files.insert(path.with_extension("lock.json"), true);
    files.insert(base.join("devcontainer-lock.json"), true);
    if let Some(features) = &raw.features {
        for feature in features
            .keys()
            .filter(|f| config::feature_ref::is_local_feature(f))
        {
            let dir = base.join(feature);
            files.insert(dir.join("devcontainer-feature.json"), false);
            files.insert(dir.join("install.sh"), false);
        }
    }
    if let Some(cas) = raw
        .customizations
        .as_ref()
        .and_then(|c| c.dcc.as_ref())
        .and_then(|d| d.registry_cas.as_ref())
    {
        for source in cas.0.values() {
            files.insert(source.path.clone(), false);
        }
    }
    for (file, optional) in files {
        record(&mut hash, file.to_string_lossy().as_bytes());
        match file_bytes(&file, optional)? {
            None => record(&mut hash, b"missing"),
            Some(bytes) => {
                record(&mut hash, b"present");
                record(&mut hash, &bytes);
                // Source bytes intentionally detect cosmetic edits too.
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    environment_names(text, &mut names);
                }
            }
        }
    }
    for name in names {
        record(&mut hash, name.as_bytes());
        match std::env::var_os(&name) {
            None => record(&mut hash, b"absent"),
            Some(value) => {
                record(&mut hash, b"present");
                record(&mut hash, value.to_string_lossy().as_bytes());
            }
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> RuntimeSnapshot {
        RuntimeSnapshot {
            protocol: 1,
            identity: "profile".into(),
            token: "attempt".into(),
            image: "sha256:image".into(),
            fingerprint: None,
            user: "dev".into(),
            workdir: "/workspace".into(),
            local_workspace: "/host".into(),
            local_cache: "/cache".into(),
            container_env: HashMap::from([("VALUE".into(), "frozen".into())]),
            attach_hooks: vec![],
            scripts: HashMap::new(),
            feature_scripts: vec![],
            ports: vec![PortMapping {
                target: 4173,
                proxy: 20000,
            }],
            memory: "8g".into(),
            cpus: "4".into(),
        }
    }
    #[test]
    fn snapshot_validation_rejects_host_authority_identity_mismatch_and_secret_errors() {
        let snapshot = sample();
        let mut value = serde_json::to_value(&snapshot).unwrap();
        let decode = |v: &serde_json::Value| {
            RuntimeSnapshot::decode(
                &serde_json::to_vec(v).unwrap(),
                "profile",
                "sha256:image",
                "attempt",
            )
        };
        assert!(decode(&value).is_ok());
        value["host_hook"] = "SECRET-host-command".into();
        let error = decode(&value).err().unwrap().to_string();
        assert!(!error.contains("SECRET"));
        value.as_object_mut().unwrap().remove("host_hook");
        value["workdir"] = "relative".into();
        assert!(decode(&value).is_err());
        value["workdir"] = "/workspace".into();
        value["ports"][0]["proxy"] = 4173.into();
        assert!(decode(&value).is_err());
        assert!(RuntimeSnapshot::decode(
            &serde_json::to_vec(&snapshot).unwrap(),
            "different",
            "sha256:image",
            "attempt"
        )
        .is_err());
        assert!(RuntimeSnapshot::decode(
            &vec![b' '; MAX_BYTES + 1],
            "profile",
            "sha256:image",
            "attempt"
        )
        .is_err());
    }
    #[test]
    fn explicit_commands_use_frozen_container_context() {
        assert_eq!(
            sample()
                .explicit_args(&[
                    "${containerEnv:VALUE}".into(),
                    "${localWorkspaceFolder}".into()
                ])
                .unwrap(),
            ["frozen", "/host"]
        );
    }
    #[test]
    fn fingerprint_detects_inherited_bytes_optional_files_and_direct_inputs() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let path = root.join("devcontainer.json");
        std::fs::write(root.join("parent.json"), r#"{"image":"debian"}"#).unwrap();
        std::fs::write(&path, r#"{"build":{"dockerfile":"Dockerfile","context":"."},"customizations":{"dcc":{"extends":"parent.json"}}}"#).unwrap();
        std::fs::write(root.join("Dockerfile"), "FROM debian\n").unwrap();
        let hash = || fingerprint(&path, root, &root.join("cache"), "").unwrap();
        let initial = hash();
        assert_eq!(initial, hash());
        std::fs::write(root.join("parent.json"), "{\"image\":\"debian\"}\n").unwrap();
        let inherited = hash();
        assert_ne!(initial, inherited);
        std::fs::write(root.join(".dockerignore"), "").unwrap();
        let optional = hash();
        assert_ne!(inherited, optional);
        std::fs::write(root.join("Dockerfile"), "FROM alpine\n").unwrap();
        assert_ne!(optional, hash());
        std::fs::write(root.join("Dockerfile"), vec![b'x'; MAX_BYTES + 1]).unwrap();
        assert!(fingerprint(&path, root, root, "").is_err());
    }
}
