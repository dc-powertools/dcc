//! Runtime discovery, frozen reuse and exact-attempt Docker creation.
use crate::{
    cache::CacheDir,
    config, docker,
    exec::ExecOptions,
    forward,
    profile::{ContainerId, ProfileName},
    runtime_snapshot::{self, RuntimeSnapshot},
    version,
    workspace::Workspace,
};
use anyhow::Context as _;
use std::path::Path;

pub(crate) struct Runtime {
    pub(crate) id: String,
    pub(crate) snapshot: RuntimeSnapshot,
    pub(crate) started: bool,
}

pub(crate) async fn reuse(
    workspace: &Workspace,
    profile: &ProfileName,
    path: &Path,
    opts: ExecOptions<'_>,
) -> anyhow::Result<Option<Runtime>> {
    let identity = ContainerId::new(workspace, profile);
    let Some(container) = docker::runtime_by_identity(identity.as_str()).await? else {
        return Ok(None);
    };
    load(container, workspace, profile, path, opts)
        .await
        .map(Some)
}

pub(crate) async fn load(
    container: docker::ContainerInspect,
    workspace: &Workspace,
    profile: &ProfileName,
    path: &Path,
    opts: ExecOptions<'_>,
) -> anyhow::Result<Runtime> {
    let identity = ContainerId::new(workspace, profile);
    version::ensure_image_version_compatible(&container.image, opts.profile_arg, opts.strict)
        .await
        .context("running container is incompatible; stop it, rebuild and recreate")?;
    let token = container
        .config
        .labels
        .get("dcc.launch_token")
        .context("running container lacks a launch snapshot; stop, rebuild and recreate")?;
    let data = docker::control(&container.id, "snapshot", None).await?;
    // Never replay stdout/stderr here: a broken snapshot/control script can contain secrets.
    anyhow::ensure!(
        data.status.success(),
        "cannot read running container snapshot; stop, rebuild and recreate"
    );
    let snapshot =
        RuntimeSnapshot::decode(&data.stdout, identity.as_str(), &container.image, token)?;
    let cache = CacheDir::new(workspace, profile);
    let metadata = docker::inspect_image_label(&container.image).await;
    let comparison = metadata.and_then(|metadata| {
        config::load_config(path, workspace, &cache, opts.strict)?;
        runtime_snapshot::fingerprint(
            path,
            &workspace.root,
            &cache.host_path,
            metadata.as_deref().unwrap_or_default(),
        )
    });
    let mut drift = false;
    let mut incomplete = false;
    match comparison {
        Ok(hash) if snapshot.fingerprint.is_some() => {
            let verified = docker::control(&container.id, "verify-config", Some(&hash)).await?;
            match verified.status.code() {
                Some(0) => anyhow::ensure!(
                    snapshot.fingerprint.as_deref() == Some(hash.as_str()),
                    "inconsistent running fingerprint protocol"
                ),
                Some(3) => drift = true,
                _ => anyhow::bail!("running configuration verification protocol failed"),
            }
        }
        _ => incomplete = true,
    }
    match docker::image_id(identity.as_image_tag().as_str()).await {
        Ok(image) => drift |= image != snapshot.image,
        Err(_) => incomplete = true,
    }
    if opts.limits.memory_explicit && opts.limits.memory != snapshot.memory {
        drift = true;
    }
    if opts.limits.cpus_explicit && opts.limits.cpus != snapshot.cpus {
        drift = true;
    }
    if drift || incomplete {
        eprintln!("warning: configuration {} for running container; using its launch configuration. Stop, rebuild if needed, and recreate to apply changes.",
            if drift { "changed" } else { "comparison is unavailable" });
    }
    let hooks = docker::control(&container.id, "wait-hooks", None).await?;
    docker::captured_ok(
        &hooks,
        "startup hooks; use docker exec for bootstrap diagnosis",
    )?;
    match docker::control(&container.id, "relay-status", None).await {
        Ok(status) if status.status.success() => (),
        Ok(status) if status.status.code() == Some(1) => eprintln!(
            "warning: port forwarding is degraded for container {}; shell access remains available",
            container.id
        ),
        _ => eprintln!("warning: forwarding health is unavailable; shell access remains available"),
    }
    if opts.keep {
        let promoted = docker::control(&container.id, "mode", Some("durable")).await?;
        docker::captured_ok(&promoted, "promote runtime to durable")?;
    }
    Ok(Runtime {
        id: container.id,
        snapshot,
        started: false,
    })
}

pub(crate) async fn forwarding_supported(args: &[String]) -> anyhow::Result<()> {
    let output = docker::capture(
        &[
            "version".into(),
            "--format".into(),
            "{{.Server.Version}}".into(),
        ],
        4096,
    )
    .await?;
    docker::captured_ok(&output, "Docker Engine version inspection")?;
    let major: u32 = std::str::from_utf8(&output.stdout)?
        .trim()
        .split('.')
        .next()
        .context("missing Engine version")?
        .parse()?;
    anyhow::ensure!(
        major >= 28,
        "forwarding requires Docker Engine 28+ for safe localhost publication; upgrade the daemon"
    );
    let mut network = "bridge";
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "-P" || arg == "--publish-all" || arg.starts_with("--publish-all=") {
            anyhow::bail!("--publish-all is incompatible with dcc forwarding");
        }
        anyhow::ensure!(
            arg != "-p"
                && !arg.starts_with("-p=")
                && arg != "--publish"
                && !arg.starts_with("--publish="),
            "custom Docker publications are incompatible with dcc forwarding"
        );
        if arg == "--network" || arg == "--net" {
            network = iter.next().context("network needs a value")?;
        } else if let Some(value) = arg
            .strip_prefix("--network=")
            .or_else(|| arg.strip_prefix("--net="))
        {
            network = value;
        }
    }
    anyhow::ensure!(
        !matches!(network, "host" | "none") && !network.starts_with("container:"),
        "forwarding requires normal bridge networking"
    );
    if network == "default" {
        network = "bridge";
    }
    let output =
        docker::capture(&["network".into(), "inspect".into(), network.into()], 65536).await?;
    docker::captured_ok(&output, "forwarding network inspection")?;
    let networks: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    anyhow::ensure!(networks.len() == 1, "ambiguous forwarding network");
    let net = &networks[0];
    anyhow::ensure!(
        net["Driver"] == "bridge",
        "forwarding requires a bridge network"
    );
    for family in ["ipv4", "ipv6"] {
        if let Some(mode) =
            net["Options"][format!("com.docker.network.bridge.gateway_mode_{family}")].as_str()
        {
            anyhow::ensure!(
                mode == "nat",
                "forwarding requires NAT bridge gateway modes"
            );
        }
    }
    Ok(())
}

pub(crate) enum Created {
    New(docker::ContainerInspect),
    Winner(docker::ContainerInspect),
}

fn never_started(container: &docker::ContainerInspect, token: &str) -> bool {
    container
        .config
        .labels
        .get("dcc.launch_token")
        .map(String::as_str)
        == Some(token)
        && container.state.status == "created"
        && container
            .state
            .started_at
            .starts_with("0001-01-01T00:00:00")
}

pub(crate) async fn create(
    args: &[String],
    name: &str,
    snapshot: &RuntimeSnapshot,
) -> anyhow::Result<Created> {
    let mut previous = None;
    for ipv6 in [true, false] {
        let mut create = vec!["create".into()];
        create.extend(forward::publication_args(&snapshot.ports, ipv6));
        create.extend(args.iter().cloned());
        let output = docker::capture(&create, 65536).await?;
        let mut attempt = if output.status.success() {
            let id = std::str::from_utf8(&output.stdout)?.trim();
            anyhow::ensure!(
                !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid created container ID"
            );
            docker::inspect_container(id).await?
        } else {
            docker::inspect_container(name).await?
        };
        if let Some(container) = &attempt {
            if container.config.labels.get("dcc.launch_token") != Some(&snapshot.token) {
                if container.config.labels.get(docker::CONTAINER_ID_LABEL)
                    == Some(&snapshot.identity)
                    && container.state.status == "running"
                {
                    return Ok(Created::Winner(
                        attempt.take().context("winner disappeared")?,
                    ));
                }
                anyhow::bail!("container name is already owned by another or not-yet-running instance; retry after it starts");
            }
        }
        let failure = if output.status.success() {
            let container = attempt
                .as_ref()
                .context("created container disappeared; refusing uncertain retry")?;
            let start = docker::capture(&["start".into(), container.id.clone()], 65536).await?;
            if start.status.success() {
                let running = docker::inspect_container(&container.id)
                    .await?
                    .context("started container disappeared")?;
                if let Err(e) = verify_publications(&running, &snapshot.ports, ipv6) {
                    docker::stop_container(&running.id)
                        .await
                        .context("cannot stop runtime with unsafe or missing publication")?;
                    return Err(e);
                }
                if !snapshot.ports.is_empty() {
                    eprintln!(
                        "dcc: ports published on the Docker daemon host's localhost{}",
                        if ipv6 {
                            " (IPv4; IPv6 where available)"
                        } else {
                            " (IPv4 only)"
                        }
                    );
                }
                return Ok(Created::New(running));
            }
            // Inspect after failure; a start response alone does not prove hooks never ran.
            attempt = docker::inspect_container(&container.id).await?;
            docker::captured_ok(&start, "container start")
                .err()
                .context("unexpected start result")?
        } else {
            docker::captured_ok(&output, "container create")
                .err()
                .context("unexpected create result")?
        };
        let safe = match &attempt {
            Some(c) => never_started(c, &snapshot.token),
            None => !output.status.success(), // no start was issued
        };
        if !ipv6 || snapshot.ports.is_empty() || !safe {
            anyhow::bail!(
                "runtime creation failed: {failure:#}{}",
                previous
                    .map(|e: String| format!("; initial attempt: {e}"))
                    .unwrap_or_default()
            );
        }
        if let Some(c) = attempt {
            let removed = docker::capture(&["rm".into(), c.id], 65536).await?;
            docker::captured_ok(&removed, "remove never-started runtime attempt")?;
        }
        eprintln!("warning: creation failed before startup; retrying once with IPv4 localhost publication only");
        previous = Some(format!("{failure:#}"));
    }
    anyhow::bail!("runtime creation exhausted attempts")
}

fn verify_publications(
    container: &docker::ContainerInspect,
    ports: &[forward::PortMapping],
    ipv6: bool,
) -> anyhow::Result<()> {
    for mapping in ports {
        let bindings = container
            .network_settings
            .ports
            .get(&format!("{}/tcp", mapping.proxy))
            .and_then(Option::as_ref)
            .context("required forwarding publication missing")?;
        let mut v4 = false;
        let mut v6 = false;
        for binding in bindings {
            let address: std::net::IpAddr = binding
                .host_ip
                .parse()
                .context("invalid published address")?;
            anyhow::ensure!(
                binding.host_port == mapping.target.to_string() && address.is_loopback(),
                "unexpected forwarding publication; stopping newly-created runtime"
            );
            v4 |= address == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
            v6 |= address == std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST);
        }
        anyhow::ensure!(v4, "required IPv4 localhost publication missing");
        if ipv6 && !v6 {
            eprintln!(
                "warning: IPv6 localhost unavailable for port {}; IPv4 remains active",
                mapping.target
            );
        }
    }
    Ok(())
}
