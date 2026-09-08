use std::{
    fmt, fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use serde::Serialize;

use crate::{
    cache::CacheDir, cli::OutputFormat, config, docker, dry_run::DryRunReport, workspace::Workspace,
};

#[derive(Debug, Clone)]
pub(crate) struct ProfileName(String);

#[derive(Debug, Clone)]
pub(crate) struct ContainerId(String);

#[derive(Debug, Clone)]
pub(crate) struct ImageTag(String);

#[derive(Debug, Clone)]
pub(crate) struct ContainerName(String);

#[derive(Debug, Eq, PartialEq, Serialize)]
struct ProfileListEntry {
    name: String,
    config: String,
    #[serde(rename = "default")]
    is_default: bool,
    running: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ProfileList {
    profiles: Vec<ProfileListEntry>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct RunningContainerView<'a> {
    container_id: &'a str,
    role: Option<&'a str>,
    name: &'a str,
}

#[derive(Debug, Eq, PartialEq)]
struct UnknownContainerRole {
    profile: String,
    container_name: String,
    role: String,
}

#[derive(Debug, Eq, PartialEq)]
enum RunningStatusAction {
    SkipEmpty,
    SkipDryRun,
    Query,
}

pub(crate) struct BootstrapOptions<'a> {
    pub(crate) image: Option<&'a str>,
    pub(crate) dockerfile: Option<&'a str>,
    pub(crate) extends: Option<&'a str>,
    pub(crate) strict: bool,
    pub(crate) dry_run: bool,
    pub(crate) debug: bool,
    pub(crate) format: OutputFormat,
}

enum BootstrapSource<'a> {
    Image(&'a str),
    Dockerfile(&'a str),
    Extends(&'a str),
}

impl ProfileName {
    pub(crate) fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns .devcontainer/<name>.json relative to workspace root.
    /// No special-casing: "devcontainer" → .devcontainer/devcontainer.json by the same rule.
    pub(crate) fn config_path(&self, workspace: &Workspace) -> PathBuf {
        workspace
            .root
            .join(".devcontainer")
            .join(format!("{}.json", self.0))
    }
}

pub(crate) async fn list_profiles(
    workspace: &Workspace,
    format: OutputFormat,
    dry_run: bool,
    debug: bool,
) -> anyhow::Result<()> {
    let mut profiles = discover_profiles(workspace)?;

    if debug {
        eprintln!("dcc debug: command `profile list`");
        eprintln!("dcc debug: workspace `{}`", workspace.root.display());
        eprintln!("dcc debug: profiles `{}`", profiles.len());
    }

    match running_status_action(profiles.len(), dry_run) {
        RunningStatusAction::SkipEmpty => {}
        RunningStatusAction::SkipDryRun => {
            eprintln!(
                "warning: profile running status is unknown because --dry-run skipped the Docker status query"
            );
        }
        RunningStatusAction::Query => match docker::running_dcc_containers().await {
            Ok(containers) => {
                let records = containers
                    .iter()
                    .map(|container| RunningContainerView {
                        container_id: container.container_id.as_str(),
                        role: container.role.as_deref(),
                        name: container.name.as_str(),
                    })
                    .collect::<Vec<_>>();
                let unknown_roles = enrich_running_statuses(workspace, &mut profiles, &records);
                if !unknown_roles.is_empty() {
                    eprintln!(
                        "warning: profile running status is unknown for one or more profiles because Docker reported an unrecognized dcc container role"
                    );
                    if debug {
                        for unknown in unknown_roles {
                            eprintln!("{}", render_unknown_role_debug(&unknown));
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!(
                    "warning: profile running status is unknown because Docker status could not be queried"
                );
                if debug {
                    eprintln!("{}", render_running_query_failure_debug(&error));
                }
            }
        },
    }

    match format {
        OutputFormat::Text => {
            print!("{}", render_text_profiles(&profiles));
        }
        OutputFormat::Json => {
            let output = ProfileList { profiles };
            let json = serde_json::to_string_pretty(&output)
                .context("failed to serialize profile list")?;
            println!("{json}");
        }
    }

    Ok(())
}

fn running_status_action(profile_count: usize, dry_run: bool) -> RunningStatusAction {
    if profile_count == 0 {
        RunningStatusAction::SkipEmpty
    } else if dry_run {
        RunningStatusAction::SkipDryRun
    } else {
        RunningStatusAction::Query
    }
}

fn enrich_running_statuses(
    workspace: &Workspace,
    profiles: &mut [ProfileListEntry],
    records: &[RunningContainerView<'_>],
) -> Vec<UnknownContainerRole> {
    let mut unresolved_roles = Vec::new();

    for profile in profiles {
        let profile_name = ProfileName::new(&profile.name);
        let container_id = ContainerId::new(workspace, &profile_name);
        let (running, unknown_records) = classify_profile_status(
            container_id.as_str(),
            records
                .iter()
                .copied()
                .filter(|record| record.container_id == container_id.as_str()),
        );
        profile.running = running;

        if running.is_none() {
            unresolved_roles.extend(unknown_records.into_iter().map(|record| {
                UnknownContainerRole {
                    profile: profile.name.clone(),
                    container_name: record.name.to_owned(),
                    role: record.role.unwrap_or_default().to_owned(),
                }
            }));
        }
    }

    unresolved_roles
}

fn classify_profile_status<'a>(
    container_id: &str,
    records: impl IntoIterator<Item = RunningContainerView<'a>>,
) -> (Option<bool>, Vec<RunningContainerView<'a>>) {
    let legacy_build_prep_name = format!("{container_id}-build-prep");
    let mut unknown_records = Vec::new();

    for record in records {
        match record.role {
            Some(docker::CONTAINER_ROLE_RUNTIME) => return (Some(true), Vec::new()),
            Some(docker::CONTAINER_ROLE_BUILD_PREP) => {}
            None | Some("") if record.name == legacy_build_prep_name => {}
            None | Some("") => return (Some(true), Vec::new()),
            Some(_) => unknown_records.push(record),
        }
    }

    if unknown_records.is_empty() {
        (Some(false), unknown_records)
    } else {
        (None, unknown_records)
    }
}

fn render_text_profiles(profiles: &[ProfileListEntry]) -> String {
    let mut output = String::new();
    for profile in profiles {
        output.push_str(&escape_one_line(&profile.name));
        if profile.is_default {
            output.push_str(" (default)");
        }
        if profile.running == Some(true) {
            output.push_str(" [running]");
        }
        output.push('\n');
    }
    output
}

fn render_unknown_role_debug(unknown: &UnknownContainerRole) -> String {
    format!(
        "dcc debug: profile `{}` has running container `{}` with unrecognized role `{}`",
        escape_one_line(&unknown.profile),
        escape_one_line(&unknown.container_name),
        escape_one_line(&unknown.role)
    )
}

fn render_running_query_failure_debug(error: &anyhow::Error) -> String {
    format!(
        "dcc debug: running container query failed: {}",
        escape_one_line(&format!("{error:#}"))
    )
}

pub(crate) fn bootstrap_profile(
    workspace: &Workspace,
    profile_arg: &str,
    opts: BootstrapOptions<'_>,
) -> anyhow::Result<()> {
    validate_direct_profile_name(profile_arg, "--profile")?;
    let profile = ProfileName::new(profile_arg);
    let config_path = profile.config_path(workspace);
    ensure_profile_absent(&config_path)?;

    let source = bootstrap_source(&opts)?;
    let config = match source {
        BootstrapSource::Image(image) => serde_json::json!({ "image": image }),
        BootstrapSource::Dockerfile(dockerfile) => {
            serde_json::json!({ "build": { "dockerfile": dockerfile } })
        }
        BootstrapSource::Extends(parent_name) => {
            validate_direct_profile_name(parent_name, "--extends")?;
            if parent_name == profile_arg {
                anyhow::bail!("--extends must name a profile other than the target profile");
            }
            let parent = ProfileName::new(parent_name);
            let parent_path = parent.config_path(workspace);
            let parent_cache = CacheDir::new(workspace, &parent);
            config::load_config(&parent_path, workspace, &parent_cache, opts.strict)
                .with_context(|| format!("failed to validate extended profile `{parent_name}`"))?;
            serde_json::json!({
                "customizations": {
                    "dcc": {
                        "extends": format!("./{parent_name}.json")
                    }
                }
            })
        }
    };
    let contents = serde_json::to_string_pretty(&config)
        .context("failed to serialize bootstrapped profile configuration")?;
    let contents = format!("{contents}\n");

    if opts.debug {
        eprintln!("dcc debug: command `profile bootstrap`");
        eprintln!("dcc debug: profile `{}`", profile.as_str());
        eprintln!("dcc debug: config `{}`", config_path.display());
    }

    if opts.dry_run {
        return DryRunReport::new(
            "profile bootstrap",
            workspace,
            &profile,
            &config_path,
            vec!["profile source validated", "profile creation planned"],
            Vec::<String>::new(),
        )
        .print(opts.format);
    }

    create_profile_exclusively(&config_path, contents.as_bytes())?;
    println!(
        "created profile `{}` at `{}`",
        profile.as_str(),
        config_path.display()
    );
    Ok(())
}

fn bootstrap_source<'a>(opts: &BootstrapOptions<'a>) -> anyhow::Result<BootstrapSource<'a>> {
    let (flag, value, source) = match (opts.image, opts.dockerfile, opts.extends) {
        (Some(value), None, None) => ("--image", value, BootstrapSource::Image(value)),
        (None, Some(value), None) => ("--dockerfile", value, BootstrapSource::Dockerfile(value)),
        (None, None, Some(value)) => ("--extends", value, BootstrapSource::Extends(value)),
        _ => anyhow::bail!(
            "profile bootstrap requires exactly one of --image, --dockerfile, or --extends"
        ),
    };
    if value.trim().is_empty() {
        anyhow::bail!("{flag} requires a non-empty value");
    }
    Ok(source)
}

fn validate_direct_profile_name(name: &str, flag: &str) -> anyhow::Result<()> {
    let path = Path::new(name);
    let direct_name = path
        .parent()
        .is_some_and(|parent| parent.as_os_str().is_empty())
        && path.file_name().is_some_and(|file_name| file_name == name);
    if name.trim().is_empty() || !direct_name {
        anyhow::bail!(
            "{flag} requires a direct profile name, not a path; profiles map to `.devcontainer/<profile>.json`"
        );
    }
    Ok(())
}

fn ensure_profile_absent(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!("profile configuration `{}` already exists", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to inspect `{}`", path.display())),
    }
}

fn create_profile_exclusively(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            anyhow::bail!("profile configuration `{}` already exists", path.display())
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to create profile configuration `{}`",
                    path.display()
                )
            });
        }
    };

    if let Err(write_error) = file.write_all(contents) {
        drop(file);
        if let Err(cleanup_error) = fs::remove_file(path) {
            anyhow::bail!(
                "failed to write profile configuration `{}`: {write_error}; failed to remove the partial file: {cleanup_error}",
                path.display()
            );
        }
        return Err(write_error).with_context(|| {
            format!("failed to write profile configuration `{}`", path.display())
        });
    }
    Ok(())
}

fn discover_profiles(workspace: &Workspace) -> anyhow::Result<Vec<ProfileListEntry>> {
    let directory = workspace.root.join(".devcontainer");
    let entries = fs::read_dir(&directory)
        .with_context(|| format!("failed to read profile directory `{}`", directory.display()))?;
    let mut profiles = Vec::new();

    for entry in entries {
        let entry = entry
            .with_context(|| format!("failed to read an entry in `{}`", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to inspect profile candidate `{}`", path.display()))?;
        let is_file = if file_type.is_symlink() {
            match fs::metadata(&path) {
                Ok(metadata) => metadata.is_file(),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to follow profile candidate `{}`", path.display())
                    });
                }
            }
        } else {
            file_type.is_file()
        };
        if !is_file {
            continue;
        }

        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        let Some(name) = file_name.strip_suffix(".json") else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        profiles.push(ProfileListEntry {
            name: name.to_owned(),
            config: format!(".devcontainer/{file_name}"),
            is_default: name == "devcontainer",
            running: None,
        });
    }

    profiles.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(profiles)
}

fn escape_one_line(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character == '\\' {
            escaped.push_str("\\\\");
        } else if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

/// Derives a profile name from the canonicalized path to a config file.
///
/// If `config` falls within the workspace, the name is derived from the path
/// relative to the workspace root. Otherwise it is derived from the absolute
/// path. In both cases non-alphanumeric characters are replaced with `-`,
/// the result is lowercased, and leading/trailing `-` are stripped.
///
/// Examples (workspace root `/proj`):
///   `/proj/.devcontainer/claude.json` → `devcontainer-claude-json`
///   `/proj/configs/dev.json`          → `configs-dev-json`
///   `/shared/base.json`               → `shared-base-json`
pub(crate) fn path_to_profile_name(config: &Path, workspace: &Workspace) -> ProfileName {
    let path_str = if config.starts_with(&workspace.root) {
        config
            .strip_prefix(&workspace.root)
            .expect("starts_with checked above")
            .to_string_lossy()
            .into_owned()
    } else {
        config.to_string_lossy().into_owned()
    };
    let slug: String = path_str
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_owned();
    ProfileName::new(slug)
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ProfileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ContainerId {
    /// Derives a stable dcc container id from the workspace identity and profile.
    ///
    /// Format: `dcc-<12hex>--<profile>` where `<12hex>` is the first 6 bytes of
    /// the SHA-256 of the workspace identity string rendered as lowercase hex.
    /// The identity is the git `origin` remote URL when available, falling back
    /// to the canonical workspace root path — so the id is identical on every
    /// machine that clones the same repository.
    pub(crate) fn new(workspace: &Workspace, profile: &ProfileName) -> Self {
        use sha2::{Digest as _, Sha256};
        let hash = Sha256::digest(workspace.identity.as_bytes());
        let hex = format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5]
        );
        Self(format!("dcc-{}--{}", hex, profile.0))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns an ImageTag with the same string. Docker image tags and container
    /// ids are separate namespaces.
    pub(crate) fn as_image_tag(&self) -> ImageTag {
        ImageTag(self.0.clone())
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContainerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ContainerName {
    pub(crate) fn resolve(configured_name: Option<&str>, fallback: &ContainerId) -> Self {
        let Some(configured_name) = configured_name else {
            return Self(fallback.0.clone());
        };
        let Some(sanitized) = sanitize_container_name(configured_name) else {
            eprintln!(
                "warning: devcontainer name `{}` cannot be used as a Docker container name; \
                 using `{}` instead",
                configured_name.trim(),
                fallback.as_str()
            );
            return Self(fallback.0.clone());
        };

        if configured_name.trim() != sanitized {
            eprintln!(
                "warning: devcontainer name `{}` was converted to Docker container name `{}`",
                configured_name.trim(),
                sanitized
            );
        }
        Self(sanitized)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

fn sanitize_container_name(name: &str) -> Option<String> {
    let mut result = String::new();
    let mut previous_dash = false;
    for ch in name.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-') {
            ch
        } else {
            '-'
        };
        if mapped == '-' {
            if previous_dash {
                continue;
            }
            previous_dash = true;
        } else {
            previous_dash = false;
        }
        result.push(mapped);
    }

    let trimmed = result
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric())
        .to_owned();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

impl fmt::Display for ContainerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ContainerName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl ImageTag {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ImageTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ImageTag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace(path: &str) -> Workspace {
        Workspace {
            root: PathBuf::from(path),
            identity: path.to_string(),
        }
    }

    fn expected_hex(identity: &str) -> String {
        use sha2::{Digest as _, Sha256};
        let hash = Sha256::digest(identity.as_bytes());
        format!(
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            hash[0], hash[1], hash[2], hash[3], hash[4], hash[5]
        )
    }

    #[test]
    fn discover_profiles_filters_direct_json_files_and_sorts_by_name() {
        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join(".devcontainer");
        fs::create_dir(&directory).unwrap();
        for name in ["zeta.json", "devcontainer.json", "alpha.json", "notes.txt"] {
            fs::write(directory.join(name), b"{}").unwrap();
        }
        fs::write(directory.join(".json"), b"{}").unwrap();
        fs::create_dir(directory.join("nested.json")).unwrap();
        let workspace = Workspace {
            root: temp.path().to_path_buf(),
            identity: "fixture".to_string(),
        };

        assert_eq!(
            discover_profiles(&workspace).unwrap(),
            vec![
                ProfileListEntry {
                    name: "alpha".to_string(),
                    config: ".devcontainer/alpha.json".to_string(),
                    is_default: false,
                    running: None,
                },
                ProfileListEntry {
                    name: "devcontainer".to_string(),
                    config: ".devcontainer/devcontainer.json".to_string(),
                    is_default: true,
                    running: None,
                },
                ProfileListEntry {
                    name: "zeta".to_string(),
                    config: ".devcontainer/zeta.json".to_string(),
                    is_default: false,
                    running: None,
                },
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn discover_profiles_includes_file_symlinks_and_skips_unselectable_entries() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt as _, os::unix::fs::symlink};

        let temp = tempfile::tempdir().unwrap();
        let directory = temp.path().join(".devcontainer");
        fs::create_dir(&directory).unwrap();
        fs::write(temp.path().join("shared.json"), b"{}").unwrap();
        symlink("../shared.json", directory.join("shared.json")).unwrap();
        symlink("../missing.json", directory.join("broken.json")).unwrap();
        fs::write(
            directory.join(OsString::from_vec(b"invalid-\xff.json".to_vec())),
            b"{}",
        )
        .unwrap();
        let workspace = Workspace {
            root: temp.path().to_path_buf(),
            identity: "fixture".to_string(),
        };

        assert_eq!(
            discover_profiles(&workspace).unwrap(),
            vec![ProfileListEntry {
                name: "shared".to_string(),
                config: ".devcontainer/shared.json".to_string(),
                is_default: false,
                running: None,
            }]
        );
    }

    #[test]
    fn text_profile_name_escapes_controls_and_backslashes() {
        assert_eq!(
            escape_one_line("normal/λ\\line\nnext\ttab"),
            "normal/λ\\\\line\\nnext\\ttab"
        );
    }

    #[test]
    fn running_status_query_policy_skips_empty_and_dry_run_lists() {
        assert_eq!(
            running_status_action(0, false),
            RunningStatusAction::SkipEmpty
        );
        assert_eq!(
            running_status_action(0, true),
            RunningStatusAction::SkipEmpty
        );
        assert_eq!(
            running_status_action(2, true),
            RunningStatusAction::SkipDryRun
        );
        assert_eq!(running_status_action(2, false), RunningStatusAction::Query);
    }

    #[test]
    fn debug_status_details_escape_every_dynamic_value() {
        let unknown = UnknownContainerRole {
            profile: "profile\nname".to_string(),
            container_name: "container\rname".to_string(),
            role: "future\\role\tvalue".to_string(),
        };
        assert_eq!(
            render_unknown_role_debug(&unknown),
            "dcc debug: profile `profile\\nname` has running container `container\\rname` with unrecognized role `future\\\\role\\tvalue`"
        );

        let error = anyhow::anyhow!("daemon\nfailed\\detail");
        assert_eq!(
            render_running_query_failure_debug(&error),
            "dcc debug: running container query failed: daemon\\nfailed\\\\detail"
        );
    }

    #[test]
    fn text_profile_rendering_composes_default_and_running_markers() {
        let profiles = vec![
            ProfileListEntry {
                name: "ci".to_string(),
                config: ".devcontainer/ci.json".to_string(),
                is_default: false,
                running: Some(true),
            },
            ProfileListEntry {
                name: "devcontainer".to_string(),
                config: ".devcontainer/devcontainer.json".to_string(),
                is_default: true,
                running: Some(true),
            },
            ProfileListEntry {
                name: "stopped".to_string(),
                config: ".devcontainer/stopped.json".to_string(),
                is_default: false,
                running: Some(false),
            },
            ProfileListEntry {
                name: "unknown\nprofile".to_string(),
                config: ".devcontainer/unknown.json".to_string(),
                is_default: false,
                running: None,
            },
        ];

        assert_eq!(
            render_text_profiles(&profiles),
            "ci [running]\ndevcontainer (default) [running]\nstopped\nunknown\\nprofile\n"
        );
    }

    #[test]
    fn json_profile_rendering_keeps_running_field_and_order_for_all_states() {
        let output = ProfileList {
            profiles: vec![
                ProfileListEntry {
                    name: "running".to_string(),
                    config: ".devcontainer/running.json".to_string(),
                    is_default: false,
                    running: Some(true),
                },
                ProfileListEntry {
                    name: "stopped".to_string(),
                    config: ".devcontainer/stopped.json".to_string(),
                    is_default: false,
                    running: Some(false),
                },
                ProfileListEntry {
                    name: "unknown".to_string(),
                    config: ".devcontainer/unknown.json".to_string(),
                    is_default: false,
                    running: None,
                },
            ],
        };

        assert_eq!(
            serde_json::to_string(&output).unwrap(),
            r#"{"profiles":[{"name":"running","config":".devcontainer/running.json","default":false,"running":true},{"name":"stopped","config":".devcontainer/stopped.json","default":false,"running":false},{"name":"unknown","config":".devcontainer/unknown.json","default":false,"running":null}]}"#
        );
    }

    #[test]
    fn running_status_classifies_current_and_legacy_roles() {
        let container_id = "dcc-abc123--ci";
        let record = |role, name| RunningContainerView {
            container_id,
            role,
            name,
        };

        assert_eq!(
            classify_profile_status(container_id, [record(Some("runtime"), "configured-name")]),
            (Some(true), Vec::new())
        );
        assert_eq!(
            classify_profile_status(
                container_id,
                [record(Some("build-prep"), "configured-name")]
            ),
            (Some(false), Vec::new())
        );
        assert_eq!(
            classify_profile_status(container_id, [record(None, "dcc-abc123--ci-build-prep")]),
            (Some(false), Vec::new())
        );
        assert_eq!(
            classify_profile_status(container_id, [record(None, "legacy-runtime")]),
            (Some(true), Vec::new())
        );
    }

    #[test]
    fn running_status_uses_running_over_unknown_over_not_running_precedence() {
        let container_id = "dcc-abc123--ci";
        let unknown = RunningContainerView {
            container_id,
            role: Some("future-role"),
            name: "future-container",
        };
        let build_prep = RunningContainerView {
            container_id,
            role: Some("build-prep"),
            name: "prep",
        };
        let runtime = RunningContainerView {
            container_id,
            role: Some("runtime"),
            name: "runtime",
        };

        assert_eq!(
            classify_profile_status(container_id, [build_prep, unknown]),
            (None, vec![unknown])
        );
        assert_eq!(
            classify_profile_status(container_id, [unknown, runtime, runtime]),
            (Some(true), Vec::new())
        );
        assert_eq!(
            classify_profile_status(container_id, [runtime, unknown]),
            (Some(true), Vec::new())
        );
    }

    #[test]
    fn container_id_basic() {
        let ws = workspace("/home/user/my-project");
        let p = ProfileName::new("claude");
        let hex = expected_hex("/home/user/my-project");
        assert_eq!(
            ContainerId::new(&ws, &p).as_str(),
            format!("dcc-{hex}--claude")
        );
    }

    #[test]
    fn container_id_default_profile() {
        let ws = workspace("/home/user/my-project");
        let p = ProfileName::new("devcontainer");
        let hex = expected_hex("/home/user/my-project");
        assert_eq!(
            ContainerId::new(&ws, &p).as_str(),
            format!("dcc-{hex}--devcontainer")
        );
    }

    #[test]
    fn container_id_root_fallback() {
        let ws = workspace("/");
        let p = ProfileName::new("dev");
        let hex = expected_hex("/");
        assert_eq!(
            ContainerId::new(&ws, &p).as_str(),
            format!("dcc-{hex}--dev")
        );
    }

    #[test]
    fn container_id_same_identity_same_id() {
        let ws1 = Workspace {
            root: PathBuf::from("/path/a"),
            identity: "https://github.com/org/repo".to_string(),
        };
        let ws2 = Workspace {
            root: PathBuf::from("/completely/different/path/b"),
            identity: "https://github.com/org/repo".to_string(),
        };
        let p = ProfileName::new("dev");
        assert_eq!(
            ContainerId::new(&ws1, &p).as_str(),
            ContainerId::new(&ws2, &p).as_str(),
            "same identity must produce the same container id regardless of root path"
        );
    }

    #[test]
    fn container_id_different_identity_different_id() {
        let ws1 = workspace("/home/user/project-a");
        let ws2 = workspace("/home/user/project-b");
        let p = ProfileName::new("dev");
        assert_ne!(
            ContainerId::new(&ws1, &p).as_str(),
            ContainerId::new(&ws2, &p).as_str(),
        );
    }

    #[test]
    fn config_path_basic() {
        let ws = workspace("/home/user/project");
        let p = ProfileName::new("claude");
        assert_eq!(
            p.config_path(&ws),
            PathBuf::from("/home/user/project/.devcontainer/claude.json")
        );
    }

    #[test]
    fn config_path_default_profile_no_special_case() {
        let ws = workspace("/home/user/project");
        let p = ProfileName::new("devcontainer");
        assert_eq!(
            p.config_path(&ws),
            PathBuf::from("/home/user/project/.devcontainer/devcontainer.json")
        );
    }

    #[test]
    fn as_image_tag_equals_container_id() {
        let ws = workspace("/home/user/project");
        let p = ProfileName::new("dev");
        let cn = ContainerId::new(&ws, &p);
        assert_eq!(cn.as_image_tag().as_str(), cn.as_str());
    }

    #[test]
    fn container_name_uses_configured_name() {
        let id = ContainerId("dcc-abc123--dev".to_string());
        let name = ContainerName::resolve(Some("example"), &id);
        assert_eq!(name.as_str(), "example");
    }

    #[test]
    fn container_name_falls_back_to_id_when_missing() {
        let id = ContainerId("dcc-abc123--dev".to_string());
        let name = ContainerName::resolve(None, &id);
        assert_eq!(name.as_str(), id.as_str());
    }

    #[test]
    fn sanitize_container_name_converts_invalid_chars() {
        assert_eq!(
            sanitize_container_name("example/project app"),
            Some("example-project-app".to_string())
        );
    }

    #[test]
    fn sanitize_container_name_keeps_valid_chars() {
        assert_eq!(
            sanitize_container_name("example_project.dev-1"),
            Some("example_project.dev-1".to_string())
        );
    }

    #[test]
    fn sanitize_container_name_trims_invalid_edges() {
        assert_eq!(
            sanitize_container_name(" --.example.-- "),
            Some("example".to_string())
        );
    }

    #[test]
    fn sanitize_container_name_collapses_repeated_dashes() {
        assert_eq!(
            sanitize_container_name("example///project"),
            Some("example-project".to_string())
        );
    }

    #[test]
    fn container_name_falls_back_when_sanitized_name_is_empty() {
        let id = ContainerId("dcc-abc123--dev".to_string());
        let name = ContainerName::resolve(Some(" /// "), &id);
        assert_eq!(name.as_str(), id.as_str());
    }

    #[test]
    fn display_matches_inner_string() {
        let p = ProfileName::new("claude");
        assert_eq!(format!("{}", p), "claude");
    }

    // --- path_to_profile_name ---

    #[test]
    fn path_name_inside_workspace() {
        let ws = workspace("/proj");
        let config = PathBuf::from("/proj/configs/dev.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "configs-dev-json"
        );
    }

    #[test]
    fn path_name_in_devcontainer_dir() {
        // The leading '.' of '.devcontainer' becomes '-', which is then trimmed,
        // so the result is "devcontainer-claude-json" not "-devcontainer-claude-json".
        let ws = workspace("/proj");
        let config = PathBuf::from("/proj/.devcontainer/claude.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "devcontainer-claude-json"
        );
    }

    #[test]
    fn path_name_outside_workspace() {
        let ws = workspace("/proj");
        let config = PathBuf::from("/shared/configs/base.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "shared-configs-base-json"
        );
    }

    #[test]
    fn path_name_nested_inside_workspace() {
        let ws = workspace("/home/user/myproject");
        let config = PathBuf::from("/home/user/myproject/a/b/c.json");
        assert_eq!(path_to_profile_name(&config, &ws).as_str(), "a-b-c-json");
    }

    #[test]
    fn path_name_special_chars_replaced() {
        let ws = workspace("/proj");
        let config = PathBuf::from("/proj/my.config/dev-2.json");
        assert_eq!(
            path_to_profile_name(&config, &ws).as_str(),
            "my-config-dev-2-json"
        );
    }
}
