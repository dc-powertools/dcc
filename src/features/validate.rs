//! Offline publication checks. This module never loads a Feature or an OCI client.
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Serialize;
use serde_json::Value;

use crate::cli::OutputFormat;

const METADATA: &str = "devcontainer-feature.json";
const UPSTREAM: &str = include_str!("../../schemas/devContainerFeature.schema.json");
const EXTENSIONS: &str = include_str!("../../schemas/dcc-feature-extensions.json");

#[derive(Serialize)]
struct Diagnostic {
    file: PathBuf,
    /// JSON Pointer, with the empty string indicating the root.
    location: String,
    message: String,
}

#[derive(Serialize)]
struct Report {
    mode: &'static str,
    valid: bool,
    files: Vec<PathBuf>,
    errors: Vec<Diagnostic>,
}

pub(crate) fn validate_path(
    path: &Path,
    upstream_only: bool,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let mut report = Report {
        mode: if upstream_only {
            "upstream-only"
        } else {
            "dcc"
        },
        valid: true,
        files: Vec::new(),
        errors: Vec::new(),
    };
    match discover(path) {
        Ok(files) => report.files = files,
        Err(error) => report.errors.push(Diagnostic {
            file: path.to_owned(),
            location: String::new(),
            message: format!("{error:#}"),
        }),
    }
    let schema = schema(upstream_only)?;
    let validator =
        jsonschema::validator_for(&schema).context("failed to compile bundled Feature schema")?;
    for file in &report.files {
        validate_file(file, &validator, upstream_only, &mut report.errors);
    }
    report.valid = report.errors.is_empty();
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Text => {
            for error in &report.errors {
                eprintln!(
                    "{}#{}: {}",
                    error.file.display(),
                    error.location,
                    error.message
                );
            }
            if report.valid {
                println!(
                    "Validated {} Feature metadata file(s) ({})",
                    report.files.len(),
                    report.mode
                );
            }
        }
    }
    if !report.valid {
        anyhow::bail!(
            "Feature validation failed ({}, {} error(s))",
            report.mode,
            report.errors.len()
        );
    }
    Ok(())
}

fn discover(path: &Path) -> anyhow::Result<Vec<PathBuf>> {
    if !std::fs::metadata(path)
        .with_context(|| format!("cannot read input {}", path.display()))?
        .is_dir()
    {
        anyhow::bail!("expected a Feature directory or collection directory");
    }
    let metadata = path.join(METADATA);
    if entry_exists(&metadata)? {
        return Ok(vec![metadata]);
    }
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(path).with_context(|| format!("cannot list {}", path.display()))?
    {
        let entry = entry.with_context(|| format!("cannot read entry in {}", path.display()))?;
        let child = entry.path();
        let kind = std::fs::metadata(&child)
            .with_context(|| format!("cannot inspect {}", child.display()))?;
        if kind.is_dir() {
            let metadata = child.join(METADATA);
            if entry_exists(&metadata)? {
                files.push(metadata);
            }
        }
    }
    files.sort();
    if files.is_empty() {
        anyhow::bail!("no Features found; expected {METADATA} here or in a direct child directory");
    }
    Ok(files)
}

// symlink_metadata distinguishes an absent file from a dangling or unreadable
// metadata entry; the latter must be attempted and reported, never skipped.
fn entry_exists(path: &Path) -> anyhow::Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("cannot inspect {}", path.display())),
    }
}

fn schema(upstream_only: bool) -> anyhow::Result<Value> {
    let mut schema: Value =
        serde_json::from_str(UPSTREAM).context("invalid bundled upstream schema")?;
    if !upstream_only {
        let extensions: Value =
            serde_json::from_str(EXTENSIONS).context("invalid bundled extensions")?;
        let replacements = extensions["replacements"]
            .as_object()
            .context("missing schema replacements")?;
        for (pointer, replacement) in replacements {
            let (parent, key) = pointer
                .rsplit_once('/')
                .context("invalid extension pointer")?;
            schema
                .pointer_mut(parent)
                .and_then(Value::as_object_mut)
                .with_context(|| format!("extension target {parent} does not exist"))?
                .insert(key.to_string(), replacement.clone());
        }
    }
    Ok(schema)
}

fn validate_file(
    file: &Path,
    validator: &jsonschema::Validator,
    upstream_only: bool,
    errors: &mut Vec<Diagnostic>,
) {
    let push = |errors: &mut Vec<Diagnostic>, location: String, message: String| {
        errors.push(Diagnostic {
            file: file.to_owned(),
            location,
            message,
        });
    };
    let bytes = match read_metadata(file) {
        Ok(bytes) => bytes,
        Err(error) => {
            push(
                errors,
                String::new(),
                format!("cannot read metadata: {error}"),
            );
            return;
        }
    };
    let value: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(error) => {
            push(errors, String::new(), format!("invalid JSON: {error}"));
            return;
        }
    };
    let before = errors.len();
    for error in validator.iter_errors(&value) {
        schema_errors(file, &error, errors);
    }
    if upstream_only || errors.len() != before {
        return;
    }
    // Deserialize the actual runtime type, with a JSON Pointer to any incompatible
    // field (including the shared project/Feature mount parser).
    let meta: super::FeatureMeta = match serde_path_to_error::deserialize(&value) {
        Ok(meta) => meta,
        Err(error) => {
            let pointer = error
                .path()
                .iter()
                .map(|segment| match segment {
                    serde_path_to_error::Segment::Seq { index } => index.to_string(),
                    serde_path_to_error::Segment::Map { key } => escape_pointer(key),
                    serde_path_to_error::Segment::Enum { variant } => escape_pointer(variant),
                    serde_path_to_error::Segment::Unknown => "?".into(),
                })
                .map(|part| format!("/{part}"))
                .collect();
            push(errors, pointer, format!("dcc parser: {}", error.inner()));
            return;
        }
    };
    // Validate declaration syntax without acquiring dependencies or probing an image.
    let mut meta = meta;
    if let Err(error) = super::normalize_dependencies(&mut meta, &file.display().to_string()) {
        push(
            errors,
            "/dependsOn".into(),
            format!("dcc compatibility: {error:#}"),
        );
    }
    let state = crate::config::vars::apply_state_path_substitutions(meta.state());
    if let Err(error) =
        crate::config::resolve::validate_state_entries_allowing_deferred_container_env(state)
    {
        push(
            errors,
            "/customizations/dcc/state".into(),
            format!("dcc compatibility: {error:#}"),
        );
    }
    // Unsafe settings are declarations, not authorization to run them. Their real
    // runtime opt-in remains enforced by build/run commands.
    if let Err(error) = super::validate_feature_meta(&file.display().to_string(), &meta, true) {
        push(
            errors,
            String::new(),
            format!("dcc compatibility: {error:#}"),
        );
    }
}

fn read_metadata(file: &Path) -> anyhow::Result<Vec<u8>> {
    let metadata = std::fs::metadata(file)?;
    anyhow::ensure!(metadata.is_file(), "expected a regular metadata file");
    Ok(std::fs::read(file)?)
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn schema_errors(
    file: &Path,
    error: &jsonschema::ValidationError<'_>,
    errors: &mut Vec<Diagnostic>,
) {
    use jsonschema::error::ValidationErrorKind;
    if let ValidationErrorKind::AnyOf { context } | ValidationErrorKind::OneOfNotValid { context } =
        error.kind()
    {
        for branch in context {
            for error in branch {
                schema_errors(file, error, errors);
            }
        }
    } else {
        errors.push(Diagnostic {
            file: file.to_owned(),
            location: error.instance_path().to_string(),
            message: format!("schema: {}", error.masked()),
        });
    }
}
