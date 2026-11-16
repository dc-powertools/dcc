//! Shared project, Feature, and image-label mount decoding.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum Mount {
    String(String),
    Object(MountObject),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MountObject {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source: String,
    target: String,
    #[serde(rename = "type")]
    mount_type: String,
}

impl<'de> Deserialize<'de> for Mount {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Dispatch explicitly so an invalid object reports its actual bad field,
        // rather than the opaque error from an untagged enum.
        let value = serde_json::Value::deserialize(deserializer)?;
        let mount = match value {
            serde_json::Value::String(value) => Self::String(value),
            serde_json::Value::Object(_) => {
                let object: MountObject =
                    serde_json::from_value(value).map_err(serde::de::Error::custom)?;
                if !matches!(object.mount_type.as_str(), "bind" | "volume") {
                    return Err(serde::de::Error::custom(
                        "object mount type must be bind or volume",
                    ));
                }
                for field in [&object.source, &object.target] {
                    if field.contains([',', '"', '\n', '\r', '\0']) {
                        return Err(serde::de::Error::custom("object mount paths cannot contain commas, quotes, or control characters"));
                    }
                }
                Self::Object(object)
            }
            _ => return Err(serde::de::Error::custom("mount must be a string or object")),
        };
        validate_mount_string(&mount.to_mount_string()).map_err(serde::de::Error::custom)?;
        Ok(mount)
    }
}

impl Mount {
    pub(crate) fn to_mount_string(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Object(object) if object.mount_type == "volume" && object.source.is_empty() => {
                format!("type=volume,target={}", object.target)
            }
            Self::Object(object) => format!(
                "type={},source={},target={}",
                object.mount_type, object.source, object.target
            ),
        }
    }
}

/// Validate the structure dcc consumes, leaving Docker-specific option values and
/// template expansion to runtime. Preserve the original string, including flags.
fn validate_mount_string(mount: &str) -> Result<(), String> {
    if mount.contains(['"', '\n', '\r', '\0']) {
        return Err("mount strings cannot contain quotes or control characters".into());
    }
    let mut mount_type = None;
    let mut source = None;
    let mut target = None;
    let mut readonly = None;
    for part in mount.split(',') {
        let part = part.trim();
        let (key, value) = part.split_once('=').unwrap_or((part, ""));
        let slot = match key {
            "type" => &mut mount_type,
            "src" | "source" => &mut source,
            "dst" | "destination" | "target" => &mut target,
            "ro" | "readonly" => {
                if !matches!(value, "" | "true" | "false" | "1" | "0") {
                    return Err("readonly must be a bare flag or a boolean".into());
                }
                &mut readonly
            }
            "" => return Err("mount contains an empty comma-separated field".into()),
            _ => continue, // Docker option passthrough, as for project mounts.
        };
        if slot.replace(value).is_some() {
            return Err(format!("duplicate mount field `{key}` (including aliases)"));
        }
    }
    if !matches!(mount_type, Some("bind" | "volume" | "tmpfs")) {
        return Err("mount requires type=bind, type=volume, or type=tmpfs".into());
    }
    if target.is_none_or(str::is_empty) {
        return Err("mount requires a nonempty target (target/dst/destination)".into());
    }
    if mount_type == Some("bind") && source.is_none_or(str::is_empty) {
        return Err("bind mount requires a nonempty source (source/src)".into());
    }
    Ok(())
}

/// Excluded targets must remain in the container's image/writable layer. Reject
/// any declared mount that would cover a target or any part of its contents.
/// Return all actual destinations for image-side symlink-ancestor preflight.
/// `run_args` uses the same supported flag grammar as the runtime sanitizer.
pub(crate) fn validate_exclusion_mounts(
    state: &[super::StateEntry],
    mounts: &[String],
    run_args: &[String],
) -> anyhow::Result<Vec<String>> {
    if state.iter().all(|entry| entry.exclusions.is_empty()) {
        return Ok(Vec::new());
    }
    let mut destinations = [
        "/workspace",
        "/cache",
        "/run/dcc",
        "/usr/local/share/dcc/rt",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    let managed_count = destinations.len();
    for mount in mounts {
        destinations.push(mount_destination(mount)?);
    }
    let mut args = run_args.iter();
    while let Some(arg) = args.next() {
        if arg == "--privileged" {
            continue;
        }
        let (flag, value) = if let Some((flag, value)) = arg.split_once('=') {
            (flag, value)
        } else {
            (
                arg.as_str(),
                args.next()
                    .ok_or_else(|| anyhow::anyhow!("runArgs flag `{arg}` requires a value"))?
                    .as_str(),
            )
        };
        match flag {
            "--mount" => destinations.push(mount_destination(value)?),
            "-v" | "--volume" => {
                let parts: Vec<_> = value.split(':').collect();
                let target = match parts.as_slice() {
                    [target] => *target,
                    [_, target] | [_, target, _] => *target,
                    _ => anyhow::bail!("cannot determine runArgs volume destination"),
                };
                destinations.push(normalize_mount_destination(target)?);
            }
            "--tmpfs" => destinations.push(normalize_mount_destination(
                value.split(':').next().unwrap_or_default(),
            )?),
            _ => (),
        }
    }
    for entry in state {
        for exclusion in &entry.exclusions {
            for destination in &destinations {
                anyhow::ensure!(
                    !paths_overlap(&exclusion.target, destination),
                    "excluded image target `{}` overlaps container mount destination `{destination}`; relocate the target outside mounted storage", exclusion.target
                );
            }
            let original = format!("{}/{}", entry.path, exclusion.path);
            // The managed workspace parent is intentionally allowed: the state
            // root is itself a nested bind. Explicit mounts cannot replace the
            // exclusion link or a directory beneath it.
            for destination in destinations.iter().skip(managed_count) {
                anyhow::ensure!(
                    !paths_overlap(&original, destination),
                    "excluded directory `{original}` overlaps explicit container mount destination `{destination}`"
                );
            }
        }
    }
    Ok(destinations)
}

fn mount_destination(mount: &str) -> anyhow::Result<String> {
    validate_mount_string(mount).map_err(anyhow::Error::msg)?;
    let target = mount
        .split(',')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| matches!(key, "dst" | "destination" | "target").then_some(value))
        .ok_or_else(|| anyhow::anyhow!("mount has no destination"))?;
    normalize_mount_destination(target)
}

fn normalize_mount_destination(path: &str) -> anyhow::Result<String> {
    anyhow::ensure!(
        path.starts_with('/')
            && !path.contains(['\0', '\n', '\r'])
            && super::vars::unresolved_variables(path).is_empty(),
        "mount destination `{path}` must resolve to an absolute container path"
    );
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    Ok(format!("/{}", parts.join("/")))
}

fn paths_overlap(a: &str, b: &str) -> bool {
    let child = |path: &str, parent: &str| {
        parent == "/"
            || path == parent
            || path
                .strip_prefix(parent)
                .is_some_and(|rest| rest.starts_with('/'))
    };
    child(a, b) || child(b, a)
}

pub(super) fn deserialize_optional_mounts<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Vec<Mount>>::deserialize(deserializer)
        .map(|mounts| mounts.map(|mounts| mounts.iter().map(Mount::to_mount_string).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn excluded(target: &str) -> Vec<super::super::StateEntry> {
        vec![serde_json::from_value(json!({
            "path":"/home/dev/.tool", "type":"directory-with-exclusions",
            "exclude":[{"path":"packages", "target":target}]
        }))
        .unwrap()]
    }

    #[test]
    fn excludes_targets_hidden_by_managed_or_explicit_mounts() {
        for target in ["/workspace/packages", "/cache/packages"] {
            assert!(validate_exclusion_mounts(&excluded(target), &[], &[]).is_err());
        }
        for mount in [
            "type=volume,target=/opt/payload",
            "type=volume,dst=/opt",
            "type=bind,src=/host,destination=/opt/payload/cache",
            "type=tmpfs,target=/opt/./payload/",
            "type=volume,target=/elsewhere/../opt//payload",
            "type=volume,target=/",
        ] {
            assert!(
                validate_exclusion_mounts(&excluded("/opt/payload"), &[mount.into()], &[]).is_err(),
                "accepted {mount}"
            );
        }
        assert!(validate_exclusion_mounts(
            &excluded("/opt/payload"),
            &["type=volume,target=/opt/payload-other".into()],
            &[]
        )
        .is_ok());
        assert!(validate_exclusion_mounts(
            &excluded("/opt/payload"),
            &["type=volume,target=${containerEnv:MISSING}".into()],
            &[]
        )
        .is_err());
    }

    #[test]
    fn excludes_targets_hidden_by_runtime_mount_volume_and_tmpfs_args() {
        for args in [
            vec!["--mount", "type=volume,target=/opt"],
            vec!["--mount=type=bind,src=/host,target=/opt/payload"],
            vec!["-v", "/host:/opt:ro"],
            vec!["--volume", "named:/opt/payload"],
            vec!["--volume=/opt/payload"],
            vec!["--tmpfs", "/opt/payload:mode=700"],
            vec!["--tmpfs=/opt/payload/cache"],
            vec!["--privileged", "--volume=named:/opt"],
        ] {
            let args: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(
                validate_exclusion_mounts(&excluded("/opt/payload"), &[], &args).is_err(),
                "accepted {args:?}"
            );
        }
        // Non-mount values cannot masquerade as flags during the scan.
        assert!(validate_exclusion_mounts(
            &excluded("/opt/payload"),
            &[],
            &[
                "--env".into(),
                "EXAMPLE=--volume=/opt".into(),
                "--volume=named:/unrelated".into()
            ]
        )
        .is_ok());
    }

    #[test]
    fn mount_preflight_returns_managed_and_explicit_destinations() {
        let destinations = validate_exclusion_mounts(
            &excluded("/opt/payload"),
            &["type=volume,target=/other".into()],
            &["--tmpfs=/scratch".into()],
        )
        .unwrap();
        assert_eq!(
            destinations,
            [
                "/workspace",
                "/cache",
                "/run/dcc",
                "/usr/local/share/dcc/rt",
                "/other",
                "/scratch"
            ]
        );
        assert!(validate_exclusion_mounts(&[], &[], &[]).unwrap().is_empty());
    }

    #[test]
    fn explicit_mount_cannot_replace_original_exclusion_link() {
        for destination in [
            "/home/dev/.tool/packages",
            "/home/dev/.tool/packages/cache",
            "/home/dev/.tool",
        ] {
            let mount = format!("type=volume,target={destination}");
            assert!(validate_exclusion_mounts(&excluded("/opt/payload"), &[mount], &[]).is_err());
        }
        let state = vec![serde_json::from_value(json!({"path":"/workspace/user-state", "type":"directory-with-exclusions", "exclude":[{"path":"packages", "target":"/opt/payload"}]})).unwrap()];
        assert!(validate_exclusion_mounts(&state, &[], &[]).is_ok());
    }

    #[test]
    fn objects_and_strings_preserve_runtime_semantics() {
        for (value, expected) in [
            (
                json!({"type":"volume","target":"/data"}),
                "type=volume,target=/data",
            ),
            (
                json!({"type":"volume","source":"data","target":"/data"}),
                "type=volume,source=data,target=/data",
            ),
            (
                json!({"type":"bind","source":"/host","target":"/data"}),
                "type=bind,source=/host,target=/data",
            ),
            (
                json!("type=bind,src=${localEnv:HOME},dst=/data,readonly"),
                "type=bind,src=${localEnv:HOME},dst=/data,readonly",
            ),
            (
                json!("type=volume,target=/data,readonly=false,volume-nocopy"),
                "type=volume,target=/data,readonly=false,volume-nocopy",
            ),
        ] {
            let mount: Mount = serde_json::from_value(value).unwrap();
            assert_eq!(mount.to_mount_string(), expected);
            let label = serde_json::to_value(&mount).unwrap();
            assert_eq!(
                serde_json::from_value::<Mount>(label)
                    .unwrap()
                    .to_mount_string(),
                expected
            );
        }
    }

    #[test]
    fn rejects_ignored_fields_and_ambiguous_or_incomplete_strings() {
        for value in [
            json!({"type":"bind","source":"/host","target":"/data","readonly":true}),
            json!({"type":"volume","target":"/data,readonly"}),
            json!("type=bind,target=/data"),
            json!("type=volume"),
            json!("type=volume,target=/data,dst=/other"),
            json!("type=volume,target=/data,readonly=maybe"),
            json!("type=volume,target=/data,readonly,ro=false"),
            json!("type=volume,target=/data,"),
            json!(42),
        ] {
            assert!(
                serde_json::from_value::<Mount>(value.clone()).is_err(),
                "{value}"
            );
        }
    }
}
