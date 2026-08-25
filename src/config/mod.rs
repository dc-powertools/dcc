use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::Context as _;
use indexmap::IndexMap;
use serde::Deserialize;

use crate::{
    cache::CacheDir,
    lifecycle::{LifecycleCommand, LifecycleHooks},
    workspace::Workspace,
};

pub(crate) mod merge;
pub(crate) mod registry_ca;
pub(crate) mod resolve;
pub(crate) mod vars;

/// The user `dcc build` runs feature install scripts as and `dcc run` passes to
/// `docker run -u` when `containerUser` is not set in the devcontainer config.
pub(crate) const DEFAULT_CONTAINER_USER: &str = "dev";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawConfig {
    pub(crate) extends: Option<String>,
    pub(crate) name: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) build: Option<BuildConfig>,
    pub(crate) features: Option<IndexMap<String, serde_json::Value>>,
    pub(crate) container_env: Option<HashMap<String, String>>,
    pub(crate) remote_env: Option<HashMap<String, String>>,
    pub(crate) container_user: Option<String>,
    pub(crate) mounts: Option<Vec<String>>,
    pub(crate) run_args: Option<Vec<String>>,
    pub(crate) privileged: Option<bool>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec_option")]
    pub(crate) cap_add: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec_option")]
    pub(crate) security_opt: Option<Vec<String>>,
    pub(crate) forward_ports: Option<Vec<u16>>,
    pub(crate) ports_attributes: Option<HashMap<String, PortAttributes>>,
    pub(crate) other_ports_attributes: Option<PortAttributes>,
    pub(crate) override_command: Option<bool>,
    #[serde(rename = "updateRemoteUserUID")]
    pub(crate) update_remote_user_uid: Option<bool>,
    pub(crate) workspace_folder: Option<String>,
    pub(crate) workspace_mount: Option<serde_json::Value>,
    pub(crate) initialize_command: Option<LifecycleCommand>,
    pub(crate) on_create_command: Option<LifecycleCommand>,
    pub(crate) update_content_command: Option<LifecycleCommand>,
    pub(crate) post_create_command: Option<LifecycleCommand>,
    pub(crate) post_start_command: Option<LifecycleCommand>,
    pub(crate) post_attach_command: Option<LifecycleCommand>,
    pub(crate) scripts: Option<HashMap<String, String>>,
    pub(crate) customizations: Option<Customizations>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildConfig {
    #[serde(default = "default_build_context")]
    pub(crate) context: String,
    #[serde(default = "default_dockerfile")]
    pub(crate) dockerfile: String,
    #[serde(default)]
    pub(crate) args: HashMap<String, BuildArgValue>,
    pub(crate) target: Option<String>,
}

fn default_build_context() -> String {
    ".".to_string()
}

fn default_dockerfile() -> String {
    "Dockerfile".to_string()
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(untagged)]
pub(crate) enum BuildArgValue {
    String(String),
    Bool(bool),
    Number(serde_json::Number),
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PortAttributes {
    pub(crate) label: Option<String>,
    pub(crate) protocol: Option<String>,
    pub(crate) on_auto_forward: Option<OnAutoForward>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum OnAutoForward {
    OpenBrowser,
    OpenBrowserOnce,
    OpenPreview,
    Silent,
    Ignore,
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct UnsafeRuntimeConfig {
    pub(crate) privileged: bool,
    pub(crate) cap_add: Vec<String>,
    pub(crate) security_opt: Vec<String>,
}

impl UnsafeRuntimeConfig {
    pub(crate) fn is_empty(&self) -> bool {
        !self.privileged && self.cap_add.is_empty() && self.security_opt.is_empty()
    }

    pub(crate) fn property_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.privileged {
            names.push("privileged");
        }
        if !self.cap_add.is_empty() {
            names.push("capAdd");
        }
        if !self.security_opt.is_empty() {
            names.push("securityOpt");
        }
        names
    }
}

fn deserialize_string_or_vec_option<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    Option::<OneOrMany>::deserialize(deserializer).map(|value| {
        value.map(|value| match value {
            OneOrMany::One(one) => vec![one],
            OneOrMany::Many(many) => many,
        })
    })
}

impl BuildArgValue {
    pub(crate) fn as_build_arg(&self) -> String {
        match self {
            Self::String(value) => value.clone(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub(crate) struct Customizations {
    pub(crate) dcc: Option<RawDccConfig>,
    #[serde(flatten)]
    pub(crate) other: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawDccConfig {
    pub(crate) extends: Option<String>,
    pub(crate) commands: Option<HashMap<String, String>>,
    pub(crate) state: Option<Vec<StateEntry>>,
    #[serde(rename = "registryCAs")]
    pub(crate) registry_cas: Option<registry_ca::RawRegistryCas>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct StateEntry {
    pub(crate) path: String,
    pub(crate) kind: StateKind,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum StateKind {
    #[default]
    Directory,
    File,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawStateEntry {
    Path(String),
    Object {
        path: String,
        #[serde(default, rename = "type")]
        kind: StateKind,
    },
}

impl<'de> Deserialize<'de> for StateEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match RawStateEntry::deserialize(deserializer)? {
            RawStateEntry::Path(path) => Ok(Self {
                path,
                kind: StateKind::Directory,
            }),
            RawStateEntry::Object { path, kind } => Ok(Self { path, kind }),
        }
    }
}

#[derive(Debug)]
pub(crate) struct DevcontainerConfig {
    pub(crate) name: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) build: Option<BuildConfig>,
    pub(crate) features: IndexMap<String, serde_json::Value>,
    pub(crate) container_env: HashMap<String, String>,
    pub(crate) remote_env: HashMap<String, String>,
    pub(crate) container_user: String,
    pub(crate) mounts: Vec<String>,
    pub(crate) run_args: Vec<String>,
    pub(crate) unsafe_runtime: UnsafeRuntimeConfig,
    pub(crate) forward_ports: Vec<u16>,
    pub(crate) ports_attributes: HashMap<String, PortAttributes>,
    pub(crate) other_ports_attributes: Option<PortAttributes>,
    pub(crate) override_command: Option<bool>,
    /// `updateRemoteUserUID` (defaults to `true`): on Linux and macOS, remap
    /// the container user's uid/gid to the host user's so bind mounts are
    /// writable regardless of the host user's uid.
    pub(crate) update_remote_user_uid: bool,
    pub(crate) workspace_folder: String,
    pub(crate) workspace_mount: Option<serde_json::Value>,
    pub(crate) initialize_command: Option<LifecycleCommand>,
    pub(crate) lifecycle: LifecycleHooks,
    pub(crate) scripts: HashMap<String, String>,
    pub(crate) state: Vec<StateEntry>,
    pub(crate) registry_cas:
        BTreeMap<registry_ca::RegistryAuthority, registry_ca::RegistryCaBundle>,
}

pub(crate) fn parse_config_file(path: &Path, strict: bool) -> anyhow::Result<RawConfig> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let mut raw: RawConfig = json5::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    check_extra_fields(&raw.extra, path, strict)?;
    check_dcc_extra_fields(&raw, path, strict)?;
    normalize_legacy_dcc_fields(&mut raw, path)?;
    if let Some(registry_cas) = raw
        .customizations
        .as_mut()
        .and_then(|customizations| customizations.dcc.as_mut())
        .and_then(|dcc| dcc.registry_cas.as_mut())
    {
        registry_cas.anchor_paths(path)?;
    }
    Ok(raw)
}

fn check_extra_fields(
    extra: &HashMap<String, serde_json::Value>,
    path: &Path,
    strict: bool,
) -> anyhow::Result<()> {
    let mut keys: Vec<&str> = extra.keys().map(|s| s.as_str()).collect();
    keys.sort();
    for key in keys {
        if strict {
            anyhow::bail!("{}: unrecognized field '{}'", path.display(), key);
        } else {
            tracing::warn!(file = %path.display(), field = %key, "unrecognized devcontainer field");
        }
    }
    Ok(())
}

fn check_dcc_extra_fields(raw: &RawConfig, path: &Path, strict: bool) -> anyhow::Result<()> {
    let Some(dcc) = raw.customizations.as_ref().and_then(|c| c.dcc.as_ref()) else {
        return Ok(());
    };
    let mut keys: Vec<&str> = dcc.extra.keys().map(|s| s.as_str()).collect();
    keys.sort_unstable();
    for key in keys {
        if strict {
            anyhow::bail!(
                "{}: unrecognized field 'customizations.dcc.{}'",
                path.display(),
                key
            );
        } else {
            tracing::warn!(
                file = %path.display(),
                field = %format!("customizations.dcc.{key}"),
                "unrecognized devcontainer field"
            );
        }
    }
    Ok(())
}

fn normalize_legacy_dcc_fields(raw: &mut RawConfig, path: &Path) -> anyhow::Result<()> {
    if raw.extends.is_none() && raw.scripts.is_none() {
        return Ok(());
    }

    let dcc = raw
        .customizations
        .get_or_insert_with(Customizations::default)
        .dcc
        .get_or_insert_with(RawDccConfig::default);

    if let Some(legacy_extends) = raw.extends.take() {
        tracing::warn!(
            file = %path.display(),
            "top-level `extends` is deprecated; use `customizations.dcc.extends`"
        );
        match &dcc.extends {
            Some(current) if current != &legacy_extends => {
                anyhow::bail!(
                    "{}: top-level `extends` conflicts with `customizations.dcc.extends`",
                    path.display()
                );
            }
            Some(_) => {}
            None => dcc.extends = Some(legacy_extends),
        }
    }

    if let Some(legacy_scripts) = raw.scripts.take() {
        tracing::warn!(
            file = %path.display(),
            "top-level `scripts` is deprecated; use `customizations.dcc.commands`"
        );
        let commands = dcc.commands.get_or_insert_with(HashMap::new);
        for (key, value) in legacy_scripts {
            commands.entry(key).or_insert(value);
        }
    }

    Ok(())
}

pub(crate) fn load_config(
    path: &Path,
    workspace: &Workspace,
    cache_dir: &CacheDir,
    strict: bool,
) -> anyhow::Result<DevcontainerConfig> {
    let mut visited = std::collections::HashSet::new();
    let raw = resolve::load_raw(path, &mut visited, strict)?;
    let config = resolve::raw_to_config(raw, path)?;
    let mut config = vars::apply_substitutions(config, workspace, cache_dir);
    warn_unsupported_compatibility_fields(&config, path);
    config.state = resolve::validate_state_entries_allowing_deferred_container_env(config.state)
        .with_context(|| format!("invalid customizations.dcc.state in `{}`", path.display()))?;
    Ok(config)
}

fn warn_unsupported_compatibility_fields(config: &DevcontainerConfig, path: &Path) {
    if config.initialize_command.is_some() {
        tracing::warn!(
            file = %path.display(),
            "initializeCommand is parsed for devcontainer compatibility, but dcc does not execute host lifecycle hooks"
        );
    }
    if config.override_command.is_some() {
        tracing::warn!(
            file = %path.display(),
            "overrideCommand is parsed for devcontainer compatibility, but dcc always uses its managed keepalive startup"
        );
    }
    if config.workspace_mount.is_some() {
        tracing::warn!(
            file = %path.display(),
            "workspaceMount is parsed for devcontainer compatibility, but dcc owns workspace mounting and will ignore it"
        );
    }
    if !workspace_folder_under_managed_workspace(&config.workspace_folder) {
        tracing::warn!(
            file = %path.display(),
            workspaceFolder = %config.workspace_folder,
            "workspaceFolder is outside /workspace; dcc will use it as the container workdir, but still mounts the project at /workspace"
        );
    }
}

fn workspace_folder_under_managed_workspace(path: &str) -> bool {
    path == vars::CONTAINER_WORKSPACE
        || path
            .strip_prefix(vars::CONTAINER_WORKSPACE)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::prelude::*;

    fn write_temp(contents: &str) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), contents).unwrap();
        file
    }

    #[test]
    fn all_known_fields() {
        let file = write_temp(
            r#"{
                "extends": "base.json",
                "name": "example",
                "image": "rust:latest",
                "build": { "context": "..", "dockerfile": "Dockerfile", "args": { "VERSION": "1", "DEBUG": false }, "target": "dev" },
                "features": { "ghcr.io/devcontainers/features/node:1": { "version": "20" } },
                "containerEnv": { "FOO": "bar" },
                "remoteEnv": { "FOO": "bar" },
                "containerUser": "dev",
                "mounts": ["type=bind,src=/tmp,dst=/tmp"],
                "runArgs": ["--add-host", "host.docker.internal:host-gateway"],
                "privileged": false,
                "capAdd": "SYS_PTRACE",
                "securityOpt": ["seccomp=unconfined"],
                "forwardPorts": [8080, 3000],
                "portsAttributes": {
                    "3000": {
                        "label": "web",
                        "protocol": "http",
                        "onAutoForward": "openBrowser"
                    }
                },
                "otherPortsAttributes": {
                    "label": "other",
                    "protocol": "https",
                    "onAutoForward": "silent"
                },
                "overrideCommand": false,
                "workspaceFolder": "${containerWorkspaceFolder}/app",
                "workspaceMount": "source=${localWorkspaceFolder},target=/workspace,type=bind",
                "initializeCommand": "echo init",
                "onCreateCommand": ["echo", "create"],
                "updateContentCommand": "echo update",
                "postCreateCommand": "echo post-create",
                "postStartCommand": "echo post-start",
                "postAttachCommand": { "a": "echo a", "b": ["echo", "b"] },
                "scripts": { "legacy": "cargo check" },
                "customizations": {
                    "dcc": {
                        "commands": { "build": "cargo build" },
                        "state": [
                            "/home/dev/.cache",
                            { "path": "/home/dev/.npmrc", "type": "file" }
                        ]
                    },
                    "vscode": { "extensions": ["rust-lang.rust-analyzer"] }
                }
            }"#,
        );
        let raw = parse_config_file(file.path(), false).unwrap();
        let dcc = raw
            .customizations
            .as_ref()
            .and_then(|c| c.dcc.as_ref())
            .expect("dcc customizations should be parsed");
        assert_eq!(raw.extends.as_deref(), None);
        assert_eq!(dcc.extends.as_deref(), Some("base.json"));
        assert_eq!(raw.name.as_deref(), Some("example"));
        assert_eq!(raw.image.as_deref(), Some("rust:latest"));
        let build = raw.build.as_ref().expect("build should be parsed");
        assert_eq!(build.context, "..");
        assert_eq!(build.dockerfile, "Dockerfile");
        assert_eq!(build.target.as_deref(), Some("dev"));
        assert_eq!(
            build.args.get("VERSION").map(BuildArgValue::as_build_arg),
            Some("1".to_string())
        );
        assert_eq!(
            build.args.get("DEBUG").map(BuildArgValue::as_build_arg),
            Some("false".to_string())
        );
        assert!(raw.features.is_some());
        assert!(raw.container_env.is_some());
        assert!(raw
            .remote_env
            .as_ref()
            .is_some_and(|m| m.get("FOO").map(|s| s.as_str()) == Some("bar")));
        assert_eq!(raw.container_user.as_deref(), Some("dev"));
        assert_eq!(
            raw.mounts.as_deref(),
            Some(&[String::from("type=bind,src=/tmp,dst=/tmp")][..])
        );
        assert_eq!(
            raw.run_args.as_deref(),
            Some(
                &[
                    String::from("--add-host"),
                    String::from("host.docker.internal:host-gateway")
                ][..]
            )
        );
        assert_eq!(raw.privileged, Some(false));
        assert_eq!(
            raw.cap_add.as_deref(),
            Some(&[String::from("SYS_PTRACE")][..])
        );
        assert_eq!(
            raw.security_opt.as_deref(),
            Some(&[String::from("seccomp=unconfined")][..])
        );
        assert_eq!(raw.forward_ports.as_deref(), Some(&[8080u16, 3000u16][..]));
        let port = raw
            .ports_attributes
            .as_ref()
            .and_then(|ports| ports.get("3000"))
            .expect("port attributes should be parsed");
        assert_eq!(port.label.as_deref(), Some("web"));
        assert_eq!(port.protocol.as_deref(), Some("http"));
        assert_eq!(port.on_auto_forward, Some(OnAutoForward::OpenBrowser));
        let other_ports = raw
            .other_ports_attributes
            .as_ref()
            .expect("otherPortsAttributes should be parsed");
        assert_eq!(other_ports.label.as_deref(), Some("other"));
        assert_eq!(other_ports.protocol.as_deref(), Some("https"));
        assert_eq!(other_ports.on_auto_forward, Some(OnAutoForward::Silent));
        assert_eq!(raw.override_command, Some(false));
        assert_eq!(
            raw.workspace_folder.as_deref(),
            Some("${containerWorkspaceFolder}/app")
        );
        assert!(raw.workspace_mount.is_some());
        assert_eq!(
            raw.initialize_command,
            Some(LifecycleCommand::Shell("echo init".to_string()))
        );
        assert_eq!(
            raw.on_create_command,
            Some(LifecycleCommand::Exec(vec![
                "echo".to_string(),
                "create".to_string()
            ]))
        );
        assert_eq!(
            raw.update_content_command,
            Some(LifecycleCommand::Shell("echo update".to_string()))
        );
        assert_eq!(
            raw.post_create_command,
            Some(LifecycleCommand::Shell("echo post-create".to_string()))
        );
        assert_eq!(
            raw.post_start_command,
            Some(LifecycleCommand::Shell("echo post-start".to_string()))
        );
        assert!(matches!(
            raw.post_attach_command,
            Some(LifecycleCommand::Parallel(_))
        ));
        assert!(raw.scripts.is_none());
        let commands = dcc.commands.as_ref().expect("commands should be parsed");
        assert_eq!(
            commands.get("legacy").map(String::as_str),
            Some("cargo check")
        );
        assert_eq!(
            commands.get("build").map(String::as_str),
            Some("cargo build")
        );
        let state = dcc.state.as_ref().expect("state should be parsed");
        assert_eq!(
            state,
            &vec![
                StateEntry {
                    path: "/home/dev/.cache".to_string(),
                    kind: StateKind::Directory,
                },
                StateEntry {
                    path: "/home/dev/.npmrc".to_string(),
                    kind: StateKind::File,
                },
            ]
        );
        assert!(raw
            .customizations
            .as_ref()
            .is_some_and(|c| c.other.contains_key("vscode")));
        assert!(raw.extra.is_empty());
    }

    #[test]
    fn remote_env_parsed() {
        let file = write_temp(r#"{ "image": "rust:1", "remoteEnv": { "TOKEN": "abc" } }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        let remote_env = raw.remote_env.expect("remoteEnv should be Some");
        assert_eq!(remote_env.get("TOKEN").map(|s| s.as_str()), Some("abc"));
    }

    #[test]
    fn jsonc_trailing_comma() {
        let file = write_temp(r#"{ "image": "rust:1", }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.image.as_deref(), Some("rust:1"));
    }

    #[test]
    fn jsonc_line_comment() {
        let file = write_temp("// a comment\n{ \"image\": \"rust:1\" }");
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.image.as_deref(), Some("rust:1"));
    }

    #[test]
    fn unknown_field_warn_mode() {
        let file = write_temp(r#"{ "fooBarBaz": "foo" }"#);
        let result = parse_config_file(file.path(), false);
        assert!(result.is_ok());
        let raw = result.unwrap();
        assert!(raw.extra.contains_key("fooBarBaz"));
    }

    #[test]
    fn unknown_field_strict_mode() {
        let file = write_temp(r#"{ "fooBarBaz": "foo" }"#);
        let result = parse_config_file(file.path(), true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("fooBarBaz"));
    }

    #[test]
    fn name_is_known_in_strict_mode() {
        let file = write_temp(r#"{ "name": "example", "image": "rust:1" }"#);
        let raw = parse_config_file(file.path(), true).unwrap();
        assert_eq!(raw.name.as_deref(), Some("example"));
        assert!(raw.extra.is_empty());
    }

    #[test]
    fn strict_accepts_final_compatibility_fields() {
        let file = write_temp(
            r#"{
                "image": "rust:1",
                "portsAttributes": {
                    "3000": { "onAutoForward": "openBrowser", "label": "web", "protocol": "http" },
                    "3001": { "onAutoForward": "openBrowserOnce" },
                    "3002": { "onAutoForward": "openPreview" },
                    "3003": { "onAutoForward": "silent" },
                    "3004": { "onAutoForward": "ignore" }
                },
                "otherPortsAttributes": { "onAutoForward": "silent" },
                "runArgs": ["--add-host", "host.docker.internal:host-gateway"],
                "overrideCommand": false,
                "workspaceFolder": "${containerWorkspaceFolder}/service",
                "workspaceMount": "source=${localWorkspaceFolder},target=/workspace,type=bind"
            }"#,
        );
        let raw = parse_config_file(file.path(), true).unwrap();
        let attrs = raw.ports_attributes.expect("portsAttributes should parse");
        assert_eq!(
            attrs.get("3000").and_then(|a| a.on_auto_forward),
            Some(OnAutoForward::OpenBrowser)
        );
        assert_eq!(
            attrs.get("3001").and_then(|a| a.on_auto_forward),
            Some(OnAutoForward::OpenBrowserOnce)
        );
        assert_eq!(
            attrs.get("3002").and_then(|a| a.on_auto_forward),
            Some(OnAutoForward::OpenPreview)
        );
        assert_eq!(
            attrs.get("3003").and_then(|a| a.on_auto_forward),
            Some(OnAutoForward::Silent)
        );
        assert_eq!(
            attrs.get("3004").and_then(|a| a.on_auto_forward),
            Some(OnAutoForward::Ignore)
        );
        assert_eq!(
            raw.other_ports_attributes.and_then(|a| a.on_auto_forward),
            Some(OnAutoForward::Silent)
        );
        assert_eq!(raw.override_command, Some(false));
        assert!(raw.workspace_mount.is_some());
        assert!(raw.extra.is_empty());
    }

    #[test]
    fn empty_object() {
        let file = write_temp("{}");
        let raw = parse_config_file(file.path(), false).unwrap();
        assert!(raw.extends.is_none());
        assert!(raw.name.is_none());
        assert!(raw.image.is_none());
        assert!(raw.features.is_none());
        assert!(raw.container_env.is_none());
        assert!(raw.remote_env.is_none());
        assert!(raw.container_user.is_none());
        assert!(raw.mounts.is_none());
        assert!(raw.run_args.is_none());
        assert!(raw.privileged.is_none());
        assert!(raw.cap_add.is_none());
        assert!(raw.security_opt.is_none());
        assert!(raw.forward_ports.is_none());
        assert!(raw.ports_attributes.is_none());
        assert!(raw.other_ports_attributes.is_none());
        assert!(raw.override_command.is_none());
        assert!(raw.workspace_folder.is_none());
        assert!(raw.workspace_mount.is_none());
        assert!(raw.initialize_command.is_none());
        assert!(raw.on_create_command.is_none());
        assert!(raw.update_content_command.is_none());
        assert!(raw.post_create_command.is_none());
        assert!(raw.post_start_command.is_none());
        assert!(raw.post_attach_command.is_none());
        assert!(raw.scripts.is_none());
        assert!(raw.customizations.is_none());
        assert!(raw.extra.is_empty());
    }

    #[test]
    fn parse_error_contains_path() {
        let file = write_temp(r#"{ "image": }"#);
        let result = parse_config_file(file.path(), false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains(&file.path().display().to_string()));
    }

    #[test]
    fn camel_case_round_trip() {
        let file = write_temp(r#"{ "forwardPorts": [80, 5432] }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        assert_eq!(raw.forward_ports, Some(vec![80u16, 5432u16]));
    }

    #[test]
    fn workspace_folder_substitutes_container_workspace_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            root: tmp.path().to_path_buf(),
            identity: "workspace".to_string(),
        };
        let cache = CacheDir::new(&workspace, &crate::profile::ProfileName::new("dev"));
        let file = write_temp(
            r#"{
                "image": "rust:1",
                "workspaceFolder": "${containerWorkspaceFolder}/service"
            }"#,
        );
        let raw = parse_config_file(file.path(), false).unwrap();
        let config = resolve::raw_to_config(raw, file.path()).unwrap();
        let config = vars::apply_substitutions(config, &workspace, &cache);
        assert_eq!(config.workspace_folder, "/workspace/service");
    }

    #[test]
    fn features_uses_index_map_preserves_order() {
        let file = write_temp(r#"{ "features": { "b": {}, "a": {} } }"#);
        let raw = parse_config_file(file.path(), false).unwrap();
        let features = raw.features.expect("features should be Some");
        let keys: Vec<&str> = features.keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, vec!["b", "a"]);
    }

    #[test]
    fn strict_accepts_customizations_dcc_and_other_namespaces() {
        let file = write_temp(
            r#"{
                "image": "rust:1",
                "customizations": {
                    "dcc": {
                        "commands": { "test": "cargo test" },
                        "state": [{ "path": "/cache/file", "type": "file" }],
                        "registryCAs": {}
                    },
                    "vscode": { "settings": {} }
                }
            }"#,
        );
        let raw = parse_config_file(file.path(), true).unwrap();
        let dcc = raw.customizations.and_then(|c| c.dcc).unwrap();
        assert_eq!(
            dcc.commands
                .as_ref()
                .and_then(|c| c.get("test"))
                .map(String::as_str),
            Some("cargo test")
        );
        assert!(dcc
            .registry_cas
            .is_some_and(|registry_cas| registry_cas.0.is_empty()));
        assert_eq!(
            dcc.state.as_ref().and_then(|s| s.first()),
            Some(&StateEntry {
                path: "/cache/file".to_string(),
                kind: StateKind::File,
            })
        );
    }

    #[test]
    fn strict_rejects_unknown_customizations_dcc_field() {
        let file = write_temp(
            r#"{
                "image": "rust:1",
                "customizations": { "dcc": { "unknownDccKey": true } }
            }"#,
        );
        let result = parse_config_file(file.path(), true);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("customizations.dcc.unknownDccKey"));
    }

    #[test]
    fn legacy_extends_and_scripts_normalized_to_dcc() {
        let file = write_temp(
            r#"{
                "extends": "base.json",
                "scripts": { "build": "cargo build" }
            }"#,
        );
        let raw = parse_config_file(file.path(), true).unwrap();
        let dcc = raw.customizations.and_then(|c| c.dcc).unwrap();
        assert!(raw.extends.is_none());
        assert!(raw.scripts.is_none());
        assert_eq!(dcc.extends.as_deref(), Some("base.json"));
        assert_eq!(
            dcc.commands
                .as_ref()
                .and_then(|c| c.get("build"))
                .map(String::as_str),
            Some("cargo build")
        );
    }

    #[test]
    fn nested_commands_win_over_legacy_scripts_in_same_file() {
        let file = write_temp(
            r#"{
                "scripts": { "build": "legacy", "test": "legacy-test" },
                "customizations": {
                    "dcc": { "commands": { "build": "nested" } }
                }
            }"#,
        );
        let raw = parse_config_file(file.path(), false).unwrap();
        let commands = raw
            .customizations
            .as_ref()
            .and_then(|c| c.dcc.as_ref())
            .and_then(|dcc| dcc.commands.as_ref())
            .unwrap();
        assert_eq!(commands.get("build").map(String::as_str), Some("nested"));
        assert_eq!(
            commands.get("test").map(String::as_str),
            Some("legacy-test")
        );
    }

    #[test]
    fn conflicting_legacy_and_nested_extends_errors() {
        let file = write_temp(
            r#"{
                "extends": "base.json",
                "customizations": { "dcc": { "extends": "other.json" } }
            }"#,
        );
        let result = parse_config_file(file.path(), false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("top-level `extends`"));
        assert!(err.to_string().contains("customizations.dcc.extends"));
    }

    #[test]
    fn same_legacy_and_nested_extends_is_allowed() {
        let file = write_temp(
            r#"{
                "extends": "base.json",
                "customizations": { "dcc": { "extends": "base.json" } }
            }"#,
        );
        let raw = parse_config_file(file.path(), false).unwrap();
        let dcc = raw.customizations.and_then(|c| c.dcc).unwrap();
        assert_eq!(dcc.extends.as_deref(), Some("base.json"));
    }

    proptest! {
        #[test]
        fn parse_config_file_never_panics(s in ".*") {
            let file = tempfile::NamedTempFile::new().unwrap();
            std::fs::write(file.path(), &s).unwrap();
            let _ = parse_config_file(file.path(), false);
        }
    }
}
