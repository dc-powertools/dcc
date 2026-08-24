use std::io::Cursor;

use anyhow::Context as _;
use indexmap::IndexMap;
use sha2::{Digest as _, Sha256};

pub(crate) struct FeatureContext {
    pub(crate) id: String,
    pub(crate) install_sh: Vec<u8>,
    pub(crate) feature_json: Vec<u8>,
    pub(crate) env_vars: IndexMap<String, String>,
    /// Environment variables to bake into the image via Dockerfile `ENV` before
    /// this feature's install script runs (`containerEnv` in the feature spec).
    pub(crate) container_env: IndexMap<String, String>,
    /// Additional files from the feature directory (e.g. helper scripts).
    /// Each entry is (filename, content, unix_mode).
    pub(crate) extra_files: Vec<(String, Vec<u8>, u32)>,
}

pub(crate) type ContextFile = (String, Vec<u8>, u32);

pub(crate) fn build_context(
    image: &str,
    devcontainer_env: &[(String, String)],
    features: &[FeatureContext],
    container_user: &str,
    provision_connector: bool,
    generated_assets: &[ContextFile],
    remap: Option<&crate::uid::RemapPlan>,
) -> anyhow::Result<Vec<u8>> {
    let dockerfile = generate_dockerfile_inner(
        image,
        devcontainer_env,
        features,
        container_user,
        provision_connector,
        !generated_assets.is_empty(),
        remap,
    );
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
        for (name, content, mode) in &feature.extra_files {
            add_to_tar(
                &mut builder,
                &format!(".dcc-features/{}/{name}", feature.id),
                content,
                *mode,
            )?;
        }
    }

    for (path, content, mode) in generated_assets {
        add_to_tar(&mut builder, path, content, *mode)?;
    }

    builder
        .finish()
        .context("failed to finish tar build context")?;
    builder
        .into_inner()
        .context("failed to retrieve tar buffer")
}

#[cfg(test)]
fn generate_dockerfile(
    image: &str,
    devcontainer_env: &[(String, String)],
    features: &[FeatureContext],
    container_user: &str,
    provision_connector: bool,
) -> String {
    generate_dockerfile_inner(
        image,
        devcontainer_env,
        features,
        container_user,
        provision_connector,
        provision_connector,
        None,
    )
}

fn generate_dockerfile_inner(
    image: &str,
    devcontainer_env: &[(String, String)],
    features: &[FeatureContext],
    container_user: &str,
    provision_connector: bool,
    install_generated_assets: bool,
    remap: Option<&crate::uid::RemapPlan>,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("FROM {image}"));
    // Stamps the dcc version that generated this Dockerfile as the first
    // post-FROM instruction. Docker's build cache keys each instruction on
    // its literal content, so bumping dcc invalidates this layer and every
    // layer after it, forcing a full rebuild of all dcc-controlled steps on
    // the next `dcc build` even though the image already exists.
    lines.push(format!(
        "LABEL dcc.version={}",
        shell_quote(env!("CARGO_PKG_VERSION"))
    ));
    if !features.is_empty() || provision_connector {
        // Suppress debconf's Dialog→Readline→Teletype→Noninteractive fallback
        // warnings that appear when apt-get runs without a controlling terminal.
        // ARG (unlike ENV) is not baked into the final image, so interactive
        // debconf tools remain usable inside the running container.
        lines.push("ARG DEBIAN_FRONTEND=noninteractive".to_string());
    }
    for (k, v) in devcontainer_env {
        lines.push(format!("ENV {}={}", k, shell_quote(v)));
    }
    // Ensure the container user exists before features are installed, so that
    // install scripts can `su` into it (see below). The `id` check makes
    // this idempotent if a feature also creates the user.
    // Skipped for root, which is guaranteed to exist in every image.
    // useradd covers Debian/Ubuntu/RHEL/Fedora; adduser -D covers Alpine/BusyBox.
    let run_as_user = (container_user != "root").then_some(container_user);
    if let Some(user) = run_as_user {
        let u = shell_quote(user);
        lines.push(format!(
            "RUN id {u} >/dev/null 2>&1 \\\n || useradd -m -s /bin/sh {u} \\\n || adduser -D -s /bin/sh {u}"
        ));
    }
    // updateRemoteUserUID remap (Linux and macOS, non-root named user, enabled
    // by default). Baked into the image as a root RUN that rewrites the user's
    // uid/gid to the host's so bind mounts are writable. See `src/uid.rs` and
    // `.meta/decisions/0002-update-remote-user-uid-in-build-stage.md`. Runs
    // immediately after user creation so features install into the
    // already-remapped home ownership.
    if let Some(crate::uid::RemapPlan::Remap { uid, gid, user }) = remap {
        lines.push(crate::uid::remap_dockerfile_block(*uid, *gid, user));
    }
    if !features.is_empty() {
        lines.push("COPY .dcc-features/ /tmp/.dcc-features/".to_string());
        // Feature install scripts run as root, per the containers.dev feature
        // spec, since most published features assume root for package
        // installs. Export the standard user env vars so a script can
        // `su "$_REMOTE_USER" -c '...'` for any setup it needs to perform as
        // containerUser (e.g. dotfiles, per-user tool installs).
        let (user, home) = match run_as_user {
            Some(user) => (user, format!("/home/{user}")),
            None => ("root", "/root".to_string()),
        };
        for (var, value) in [
            ("_REMOTE_USER", user),
            ("_CONTAINER_USER", user),
            ("_REMOTE_USER_HOME", home.as_str()),
            ("_CONTAINER_USER_HOME", home.as_str()),
        ] {
            lines.push(format!("ENV {var}={}", shell_quote(value)));
        }
        for f in features {
            // containerEnv: bake into the image layer before this feature runs
            for (k, v) in &f.container_env {
                lines.push(format!("ENV {}={}", k, shell_quote(v)));
            }
            let feature_dir = format!("/tmp/.dcc-features/{}", f.id);
            let install_path = format!("{feature_dir}/install.sh");
            if f.env_vars.is_empty() {
                lines.push(format!(
                    "RUN chmod +x {install_path} \\\n && cd {feature_dir} \\\n && ./install.sh"
                ));
            } else {
                let env_prefix: String = f
                    .env_vars
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, shell_quote(v)))
                    .collect::<Vec<_>>()
                    .join(" ");
                lines.push(format!(
                    "RUN chmod +x {install_path} \\\n && cd {feature_dir} \\\n && {env_prefix} \\\n    ./install.sh"
                ));
            }
        }
        lines.push("RUN rm -rf /tmp/.dcc-features/".to_string());
    }
    if install_generated_assets {
        lines.push("COPY .dcc-generated/ /usr/local/share/dcc/".to_string());
        lines.push("RUN chmod +x /usr/local/share/dcc/dcc-*".to_string());
    }
    // Validate the baked port-forward connector after features have run. A compatible
    // tool supplied by the base image or a feature needs no package installation;
    // otherwise try the distro-specific OpenBSD netcat/Nmap Ncat packages in turn.
    if provision_connector {
        lines.push(
            "RUN ( /usr/local/share/dcc/dcc-connect --check >/dev/null 2>&1 \
             \\\n || (command -v apt-get >/dev/null 2>&1 \
             && apt-get update -qq \
             && apt-get install -y --no-install-recommends netcat-openbsd) \
             \\\n || (command -v apk >/dev/null 2>&1 \
             && apk add --no-cache netcat-openbsd) \
             \\\n || (command -v yum >/dev/null 2>&1 \
             && yum install -y nmap-ncat) \
             \\\n || (command -v dnf >/dev/null 2>&1 \
             && dnf install -y nmap-ncat) \
             \\\n || (printf '%s\\n' 'dcc: unable to install a compatible port-forward connector' >&2; false) ) \
             \\\n && /usr/local/share/dcc/dcc-connect --check"
                .to_string(),
        );
    }
    lines.join("\n") + "\n"
}

pub(crate) fn shell_quote(value: &str) -> String {
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
    use std::io::Read as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;

    fn archived_install_script(script: &[u8]) -> Vec<u8> {
        let feature = FeatureContext {
            id: "test-feature".to_string(),
            install_sh: script.to_vec(),
            feature_json: b"{}".to_vec(),
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let tar_bytes = build_context("rust:1", &[], &[feature], "root", false, &[], None).unwrap();
        let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            if entry.path().unwrap()
                == std::path::Path::new(".dcc-features/test-feature/install.sh")
            {
                let mut content = Vec::new();
                entry.read_to_end(&mut content).unwrap();
                return content;
            }
        }
        panic!("install.sh not found in generated build context");
    }

    fn run_install_script(script: &[u8], secret: &str) -> std::process::Output {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("install.sh");
        std::fs::write(&path, script).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();

        Command::new("/bin/sh")
            .args(["-c", "./install.sh"])
            .current_dir(temp.path())
            .env("FEATURE_SECRET", secret)
            .output()
            .unwrap()
    }

    fn assert_shell_quote_round_trip(value: &str) {
        let script = format!("printf '%s' {}", shell_quote(value));
        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "shell rejected quoted argument {value:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, value.as_bytes());
    }

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
    fn build_context_includes_stage_label_feature_and_generated_assets() {
        let feature = FeatureContext {
            id: "matrix-feature".to_string(),
            install_sh: b"#!/bin/sh\n".to_vec(),
            feature_json: b"{}".to_vec(),
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let context = build_context(
            "alpine:3",
            &[],
            &[feature],
            "root",
            true,
            &[crate::forward::baked_connector_asset()],
            None,
        )
        .unwrap();
        let mut archive = tar::Archive::new(std::io::Cursor::new(&context));
        let mut paths = Vec::new();
        let mut dockerfile = String::new();
        let mut connector_mode = None;
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            if path == "Dockerfile" {
                std::io::Read::read_to_string(&mut entry, &mut dockerfile).unwrap();
            } else if path == ".dcc-generated/dcc-connect" {
                connector_mode = Some(entry.header().mode().unwrap());
            }
            paths.push(path);
        }
        assert!(paths.contains(&".dcc-features/matrix-feature/install.sh".to_string()));
        assert!(paths.contains(&".dcc-generated/dcc-connect".to_string()));
        assert_eq!(connector_mode, Some(0o755));
        assert_eq!(dockerfile.lines().next(), Some("FROM alpine:3"));
        assert!(dockerfile
            .lines()
            .any(|line| line.starts_with("LABEL dcc.version=")));
        assert!(dockerfile.contains("COPY .dcc-generated/ /usr/local/share/dcc/"));
        let feature_pos = dockerfile.find("matrix-feature/install.sh").unwrap();
        let connector_pos = dockerfile.find("dcc-connect --check").unwrap();
        assert!(
            feature_pos < connector_pos,
            "Feature installation must precede fallback package installation: {dockerfile}"
        );
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
    fn shell_quote_round_trips_edge_matrix_through_sh() {
        for value in [
            "",
            "plain",
            " leading and trailing ",
            "single'quote",
            "$HOME $(touch nope) `false`",
            "line one\nline two",
            "tabs\tand\\backslashes",
        ] {
            assert_shell_quote_round_trip(value);
        }
    }

    use proptest::prelude::*;
    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 96,
            failure_persistence: None,
            rng_seed: proptest::test_runner::RngSeed::Fixed(0x5A11_0053),
            ..ProptestConfig::default()
        })]

        #[test]
        fn shell_quote_round_trips_bounded_ascii_through_sh(
            bytes in proptest::collection::vec(1u8..=126, 0..48),
        ) {
            let value = String::from_utf8(bytes).unwrap();
            let script = format!("printf '%s' {}", shell_quote(&value));
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg(script)
                .output()
                .unwrap();
            prop_assert!(output.status.success());
            prop_assert_eq!(output.stdout, value.as_bytes());
        }
    }

    #[test]
    fn dockerfile_no_features_with_user() {
        let df = generate_dockerfile("rust:1", &[], &[], "dev", false);
        assert!(df.contains("FROM rust:1"));
        assert!(df.contains("id 'dev'"));
        assert!(df.contains("useradd"));
        assert!(df.contains("adduser"));
    }

    #[test]
    fn dockerfile_root_user_skips_creation() {
        let df = generate_dockerfile("rust:1", &[], &[], "root", false);
        assert!(df
            .lines()
            .any(|line| line == format!("LABEL dcc.version='{}'", env!("CARGO_PKG_VERSION"))));
        assert!(
            !df.contains("useradd") && !df.contains("adduser") && !df.contains("id 'root'"),
            "root must not trigger user creation: {df}"
        );
    }

    fn remap_plan(uid: u32, gid: u32, user: &str) -> crate::uid::RemapPlan {
        crate::uid::RemapPlan::Remap {
            uid,
            gid,
            user: user.to_string(),
        }
    }

    #[test]
    fn dockerfile_emits_remap_block_when_planned() {
        let df = generate_dockerfile_inner(
            "rust:1",
            &[],
            &[],
            "dev",
            false,
            false,
            Some(&remap_plan(1001, 999, "dev")),
        );
        assert!(df.contains("ARG REMOTE_USER='dev'"), "got: {df}");
        assert!(df.contains("ARG NEW_UID=1001"), "got: {df}");
        assert!(df.contains("ARG NEW_GID=999"), "got: {df}");
        assert!(df.contains("updateRemoteUserUID"), "got: {df}");
        // Remap runs immediately after user creation, before any feature COPY.
        let user_pos = df.find("id 'dev'").unwrap();
        let remap_pos = df.find("ARG REMOTE_USER=").unwrap();
        assert!(
            remap_pos > user_pos,
            "remap must follow user creation, got: {df}"
        );
    }

    #[test]
    fn dockerfile_omits_remap_block_when_none() {
        let df = generate_dockerfile_inner("rust:1", &[], &[], "dev", false, false, None);
        assert!(!df.contains("ARG REMOTE_USER="), "got: {df}");
        assert!(!df.contains("updateRemoteUserUID"), "got: {df}");
    }

    #[test]
    fn dockerfile_user_creation_before_features() {
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "dev", false);
        let id_pos = df.find("id 'dev'").unwrap();
        let copy_pos = df.find("COPY").unwrap();
        assert!(
            id_pos < copy_pos,
            "user creation should appear before features are copied in"
        );
    }

    #[test]
    fn dockerfile_feature_copy_not_chowned() {
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "dev", false);
        assert!(df.contains("COPY .dcc-features/ /tmp/.dcc-features/"));
        assert!(!df.contains("--chown"));
        assert!(!df.contains("USER "));
    }

    #[test]
    fn dockerfile_feature_install_exports_remote_user_env() {
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "dev", false);
        assert!(df.contains("ENV _REMOTE_USER='dev'"));
        assert!(df.contains("ENV _CONTAINER_USER='dev'"));
        assert!(df.contains("ENV _REMOTE_USER_HOME='/home/dev'"));
        assert!(df.contains("ENV _CONTAINER_USER_HOME='/home/dev'"));

        let env_pos = df.find("ENV _REMOTE_USER=").unwrap();
        let install_pos = df.find("RUN chmod +x").unwrap();
        assert!(
            env_pos < install_pos,
            "_REMOTE_USER must be exported before the install RUN"
        );
    }

    #[test]
    fn dockerfile_feature_install_runs_as_root_with_root_user_env() {
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "root", false);
        assert!(!df.contains("USER "));
        assert!(!df.contains("--chown"));
        assert!(df.contains("COPY .dcc-features/ /tmp/.dcc-features/"));
        assert!(df.contains("ENV _REMOTE_USER='root'"));
        assert!(df.contains("ENV _CONTAINER_USER='root'"));
        assert!(df.contains("ENV _REMOTE_USER_HOME='/root'"));
        assert!(df.contains("ENV _CONTAINER_USER_HOME='/root'"));
    }

    #[test]
    fn dockerfile_one_feature_no_env() {
        let f = FeatureContext {
            id: "my-feature".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "root", false);
        assert!(df.contains("FROM rust:1"));
        assert!(df.contains("COPY .dcc-features/"));
        assert!(df.contains("chmod +x /tmp/.dcc-features/my-feature/install.sh"));
        assert!(df.contains("RUN rm -rf /tmp/.dcc-features/"));
        // No env vars prefix before ./install.sh
        assert!(df.contains(" && ./install.sh"));
    }

    #[test]
    fn dockerfile_feature_container_env_emitted_before_run() {
        let mut container_env = IndexMap::new();
        container_env.insert("MY_VAR".to_string(), "hello".to_string());
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env,
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "root", false);
        let env_pos = df.find("ENV MY_VAR=").unwrap();
        let run_pos = df.find("RUN chmod +x").unwrap();
        assert!(env_pos < run_pos, "ENV must appear before RUN");
        assert!(df.contains("ENV MY_VAR='hello'"));
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
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "root", false);
        assert!(df.contains("VERSION='20'"));
    }

    #[test]
    fn dockerfile_debian_frontend_set_when_features_present() {
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[f], "root", false);
        assert!(
            df.contains("ARG DEBIAN_FRONTEND=noninteractive"),
            "DEBIAN_FRONTEND ARG must be present when features are installed"
        );
    }

    #[test]
    fn dockerfile_debian_frontend_set_when_provisioning_connector() {
        let df = generate_dockerfile("rust:1", &[], &[], "root", true);
        assert!(
            df.contains("ARG DEBIAN_FRONTEND=noninteractive"),
            "DEBIAN_FRONTEND ARG must be present when nc is installed"
        );
    }

    #[test]
    fn dockerfile_debian_frontend_absent_when_no_apt_steps() {
        let df = generate_dockerfile("rust:1", &[], &[], "root", false);
        assert!(
            !df.contains("DEBIAN_FRONTEND"),
            "DEBIAN_FRONTEND must not appear when no apt-get steps are generated"
        );
    }

    #[test]
    fn dockerfile_installs_nc_after_features() {
        let feature = FeatureContext {
            id: "ordering-fixture".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &[], &[feature], "root", true);
        assert_eq!(
            df.matches("/usr/local/share/dcc/dcc-connect --check")
                .count(),
            2,
            "connector must be checked before and after fallbacks: {df}"
        );
        assert!(
            !df.contains("RUN command -v nc"),
            "an arbitrary nc must not bypass capability checks: {df}"
        );
        assert!(
            df.contains("netcat-openbsd"),
            "apt/apk package should be named"
        );
        assert!(df.contains("nmap-ncat"), "yum/dnf package should be named");
        let feature_pos = df.find("ordering-fixture/install.sh").unwrap();
        let copy_pos = df.find("COPY .dcc-generated/").unwrap();
        let connector_pos = df.find("dcc-connect --check").unwrap();
        let apt_pos = df.find("command -v apt-get").unwrap();
        assert!(
            feature_pos < copy_pos && copy_pos < connector_pos && connector_pos < apt_pos,
            "features and the baked wrapper must precede connector provisioning: {df}"
        );
    }

    #[test]
    fn dockerfile_provisions_connector_after_user_creation() {
        let df = generate_dockerfile("rust:1", &[], &[], "dev", true);
        let user_pos = df.find("id 'dev'").unwrap();
        let connector_pos = df.find("dcc-connect --check").unwrap();
        assert!(
            connector_pos > user_pos,
            "connector provisioning should appear after user creation"
        );
    }

    #[test]
    fn dockerfile_omits_connector_provisioning_when_not_requested() {
        let df = generate_dockerfile("rust:1", &[], &[], "root", false);
        assert!(
            !df.contains("dcc-connect --check"),
            "connector provisioning should be absent"
        );
    }

    #[test]
    fn build_context_preserves_feature_install_script_bytes() {
        for script in [
            b"#!/bin/sh\nprintf 'posix'\n".as_slice(),
            b"#!/usr/bin/env bash\nprintf 'bash'\n".as_slice(),
            b"printf 'no-shebang'\n".as_slice(),
            b"".as_slice(),
        ] {
            assert_eq!(archived_install_script(script), script);
        }
    }

    #[test]
    fn generated_feature_install_scripts_execute_with_their_interpreter_contract() {
        for script in [
            b"#!/bin/sh\ntest \"$FEATURE_SECRET\" = expected\n".as_slice(),
            b"#!/usr/bin/env bash\n[[ \"$FEATURE_SECRET\" == expected ]]\n".as_slice(),
            b"test \"$FEATURE_SECRET\" = expected\n".as_slice(),
        ] {
            let output = run_install_script(&archived_install_script(script), "expected");
            assert!(
                output.status.success(),
                "generated install script failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    fn default_feature_install_does_not_trace_expanded_secrets() {
        const SECRET: &str = "fixture-secret-that-must-not-be-traced";
        let script = archived_install_script(b"#!/bin/sh\n: \"$FEATURE_SECRET\"\nprintf done\n");
        let output = run_install_script(&script, SECRET);
        assert!(output.status.success());
        assert_eq!(output.stdout, b"done");
        assert!(!String::from_utf8_lossy(&output.stderr).contains(SECRET));

        // Negative control: the same command does expose the expanded value when
        // unconditional xtrace is reintroduced, so the assertion above detects the
        // pre-change failure mode rather than merely observing an unused variable.
        let traced = b"#!/bin/sh\nset -x\n: \"$FEATURE_SECRET\"\nprintf done\n";
        let traced_output = run_install_script(traced, SECRET);
        assert!(String::from_utf8_lossy(&traced_output.stderr).contains(SECRET));
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
            container_env: IndexMap::new(),
            extra_files: vec![],
        };
        let tar_bytes = build_context("rust:1", &[], &[f], "root", false, &[], None).unwrap();
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

    #[test]
    fn extra_files_included_in_tar() {
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: b"#!/bin/sh\n./helper.sh\n".to_vec(),
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env: IndexMap::new(),
            extra_files: vec![(
                "helper.sh".to_string(),
                b"#!/bin/sh\necho hi\n".to_vec(),
                0o755,
            )],
        };
        let tar_bytes = build_context("rust:1", &[], &[f], "root", false, &[], None).unwrap();

        let mut archive = tar::Archive::new(std::io::Cursor::new(&tar_bytes));
        let mut found_helper = false;
        for entry in archive.entries().unwrap() {
            let entry = entry.unwrap();
            let path = entry.path().unwrap().to_str().unwrap().to_owned();
            if path == ".dcc-features/feat/helper.sh" {
                found_helper = true;
            }
        }
        assert!(found_helper, "helper.sh should be present in the tar");
    }

    #[test]
    fn devcontainer_env_appears_before_feature_env() {
        let devcontainer_env = vec![("DC_VAR".to_string(), "dc_value".to_string())];
        let mut container_env = IndexMap::new();
        container_env.insert("FEAT_VAR".to_string(), "feat_value".to_string());
        let f = FeatureContext {
            id: "feat".to_string(),
            install_sh: vec![],
            feature_json: vec![],
            env_vars: IndexMap::new(),
            container_env,
            extra_files: vec![],
        };
        let df = generate_dockerfile("rust:1", &devcontainer_env, &[f], "root", false);
        let dc_pos = df.find("ENV DC_VAR=").unwrap();
        let feat_pos = df.find("ENV FEAT_VAR=").unwrap();
        assert!(
            dc_pos < feat_pos,
            "devcontainer ENV should appear before feature ENV"
        );
    }
}
