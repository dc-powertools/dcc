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
