use std::io::Cursor;

use anyhow::Context as _;
use indexmap::IndexMap;
use sha2::{Digest as _, Sha256};

pub(crate) struct FeatureContext {
    pub(crate) id: String,
    pub(crate) install_sh: Vec<u8>,
    pub(crate) feature_json: Vec<u8>,
    pub(crate) env_vars: IndexMap<String, String>,
}

pub(crate) fn build_context(
    image: &str,
    features: &[FeatureContext],
    container_user: Option<&str>,
) -> anyhow::Result<Vec<u8>> {
    let dockerfile = generate_dockerfile(image, features, container_user);
    let mut builder = tar::Builder::new(Vec::new());

    add_to_tar(&mut builder, "Dockerfile", dockerfile.as_bytes(), 0o644)?;

    for feature in features {
        add_to_tar(
            &mut builder,
            &format!(".dcc-features/{}/install.sh", feature.id),
            &feature.install_sh,
            0o755,
        )?;
        add_to_tar(
            &mut builder,
            &format!(".dcc-features/{}/devcontainer-feature.json", feature.id),
            &feature.feature_json,
            0o644,
        )?;
    }

    builder
        .finish()
        .context("failed to finish tar build context")?;
    builder
        .into_inner()
        .context("failed to retrieve tar buffer")
}

fn generate_dockerfile(
    image: &str,
    features: &[FeatureContext],
    container_user: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("FROM {image}"));
    if !features.is_empty() {
        lines.push("COPY .dcc-features/ /tmp/.dcc-features/".to_string());
        for f in features {
            let install_path = format!("/tmp/.dcc-features/{}/install.sh", f.id);
            if f.env_vars.is_empty() {
                lines.push(format!(
                    "RUN chmod +x {install_path} \\\n && {install_path}"
                ));
            } else {
                let env_prefix: String = f
                    .env_vars
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, shell_quote(v)))
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(format!(
                    "RUN chmod +x {install_path} \\\n && {env_prefix} \\\n    {install_path}"
                ));
            }
        }
        lines.push("RUN rm -rf /tmp/.dcc-features/".to_string());
    }
    // Ensure the container user exists. Runs after features so that a feature
    // that already creates the user doesn't conflict (id check makes it idempotent).
    // Skipped for root, which is guaranteed to exist in every image.
    // useradd covers Debian/Ubuntu/RHEL/Fedora; adduser -D covers Alpine/BusyBox.
    if let Some(user) = container_user {
        if user != "root" {
            let u = shell_quote(user);
            lines.push(format!(
                "RUN id {u} >/dev/null 2>&1 \\\n || useradd -m -s /bin/sh {u} \\\n || adduser -D -s /bin/sh {u}"
            ));
        }
    }
    lines.join("\n") + "\n"
}

fn shell_quote(value: &str) -> String {
    // Wrap in single quotes, escape embedded single quotes as '\''
    let inner = value.replace('\'', r"'\''");
    format!("'{inner}'")
}

fn feature_slug(reference: &str) -> String {
    reference
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// Returns a unique slug for `reference`, avoiding collisions with `seen`.
pub(crate) fn unique_feature_id(
    reference: &str,
    seen: &mut std::collections::HashSet<String>,
) -> String {
    let base = feature_slug(reference);
    if seen.insert(base.clone()) {
        return base;
    }
    // Collision: append first 4 bytes of SHA-256 as 8 hex chars
    let hash = Sha256::digest(reference.as_bytes());
    let suffix = format!(
        "{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3]
    );
    let id = format!("{base}-{suffix}");
    seen.insert(id.clone());
    id
}

fn add_to_tar(
    builder: &mut tar::Builder<Vec<u8>>,
    path: &str,
    data: &[u8],
    mode: u32,
) -> anyhow::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(mode);
    header.set_cksum();
    builder
        .append_data(&mut header, path, Cursor::new(data))
        .with_context(|| format!("failed to add {path} to build context tar"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn feature_slug_basic() {
        assert_eq!(
            feature_slug("ghcr.io/devcontainers/features/node:1"),
            "ghcr-io-devcontainers-features-node-1"
        );
    }

    #[test]
    fn feature_slug_all_non_alnum() {
        assert_eq!(feature_slug("..."), "---");
    }

    #[test]
    fn unique_feature_id_no_collision() {
        let mut seen = HashSet::new();
        let id = unique_feature_id("ghcr.io/foo/bar:1", &mut seen);
        assert_eq!(id, "ghcr-io-foo-bar-1");
    }

    #[test]
    fn unique_feature_id_collision_appends_hash() {
        let mut seen = HashSet::new();
        // First one takes the base slug
        let id1 = unique_feature_id("ref-one", &mut seen);
        // A different reference that produces the same slug (manually craft it)
        // "ref.one" also slugifies to "ref-one"
        let id2 = unique_feature_id("ref.one", &mut seen);
        assert_eq!(id1, "ref-one");
        assert_ne!(id2, "ref-one");
        assert_eq!(id2.len(), "ref-one".len() + 9); // "-" + 8 hex chars
    }

    #[test]
    fn shell_quote_empty() {
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn shell_quote_simple() {
        assert_eq!(shell_quote("20"), "'20'");
    }

    #[test]
    fn shell_quote_spaces() {
        assert_eq!(shell_quote("hello world"), "'hello world'");
    }

    #[test]
    fn shell_quote_single_quote() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn shell_quote_dollar_sign_neutralised() {
        // Dollar signs are safe inside single quotes
        let q = shell_quote("$(evil)");
        assert_eq!(q, "'$(evil)'");
    }

    #[test]
    fn dockerfile_no_features_no_user() {
        let df = generate_dockerfile("rust:1", &[], None);
        assert_eq!(df, "FROM rust:1\n");
    }

    #[test]
    fn dockerfile_no_features_with_user() {
        let df = generate_dockerfile("rust:1", &[], Some("dev"));
        assert!(df.contains("FROM rust:1"));
        assert!(df.contains("id 'dev'"));
        assert!(df.contains("useradd"));
        assert!(df.contains("adduser"));
    }

    #[test]
    fn dockerfile_root_user_skips_creation() {
        let df = generate_dockerfile("rust:1", &[], Some("root"));
        assert_eq!(df, "FROM rust:1\n");
    }

    #[test]
    fn dockerfile_user_creation_after_features() {
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
        };
        let df = generate_dockerfile("rust:1", &[f], Some("dev"));
        let rm_pos = df.find("rm -rf /tmp/.dcc-features/").unwrap();
        let id_pos = df.find("id 'dev'").unwrap();
        assert!(
            id_pos > rm_pos,
            "user creation should appear after feature cleanup"
        );
    }

    #[test]
    fn dockerfile_one_feature_no_env() {
        let f = FeatureContext {
            id: "my-feature".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
        };
        let df = generate_dockerfile("rust:1", &[f], None);
        assert!(df.contains("FROM rust:1"));
        assert!(df.contains("COPY .dcc-features/"));
        assert!(df.contains("chmod +x /tmp/.dcc-features/my-feature/install.sh"));
        assert!(df.contains("RUN rm -rf /tmp/.dcc-features/"));
        // No env vars prefix
        assert!(!df.contains("="));
    }

    #[test]
    fn dockerfile_one_feature_with_env() {
        let mut env = IndexMap::new();
        env.insert("VERSION".to_string(), "20".to_string());
        let f = FeatureContext {
            id: "node".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: env,
        };
        let df = generate_dockerfile("rust:1", &[f], None);
        assert!(df.contains("VERSION='20'"));
    }

    #[test]
    fn build_context_tar_roundtrip() {
        let mut env = IndexMap::new();
        env.insert("VERSION".to_string(), "lts".to_string());
        let f = FeatureContext {
            id: "node".to_string(),
            install_sh: b"#!/bin/sh\necho hello\n".to_vec(),
            feature_json: b"{}".to_vec(),
            env_vars: env,
        };
        let tar_bytes = build_context("rust:1", &[f], None).unwrap();
        assert!(!tar_bytes.is_empty());

        // Extract and verify
        let mut archive = tar::Archive::new(std::io::Cursor::new(&tar_bytes));
        let mut found_dockerfile = false;
        let mut found_install = false;
        let mut found_json = false;
        for entry in archive.entries().unwrap() {
            let entry = entry.unwrap();
            let path = entry.path().unwrap().to_str().unwrap().to_owned();
            match path.as_str() {
                "Dockerfile" => found_dockerfile = true,
                ".dcc-features/node/install.sh" => found_install = true,
                ".dcc-features/node/devcontainer-feature.json" => found_json = true,
                _ => {}
            }
        }
        assert!(found_dockerfile, "Dockerfile missing from tar");
        assert!(found_install, "install.sh missing from tar");
        assert!(found_json, "devcontainer-feature.json missing from tar");
    }
}
