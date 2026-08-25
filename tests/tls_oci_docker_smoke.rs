mod common;
use common::*;

use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair};
use rustls::{
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
    ServerConnection, StreamOwned,
};
use sha2::{Digest as _, Sha256};
use std::{
    io::Write as _,
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

const IMAGE: &str = "debian:bookworm-slim";
const FEATURE_MARKER: &str = "dcc-tls-oci-feature-installed";
const FEATURE_REPOSITORY: &str = "dcc/smoke-feature";
const FEATURE_TAG: &str = "1";
const MAX_RECORDED_REQUESTS: usize = 8;

struct SmokeFixture {
    fx: Fixture,
}

impl SmokeFixture {
    fn new() -> Self {
        Self { fx: Fixture::new() }
    }

    fn write_config(&self, content: &str) {
        self.fx.write_config("devcontainer.json", content);
    }

    fn dcc(&self, args: &[&str]) -> Output {
        self.fx.dcc(args).output().expect("failed to run dcc")
    }

    fn container_id(&self) -> String {
        let output = self.dcc(&["id"]);
        assert_success(&output);
        String::from_utf8(output.stdout)
            .expect("container id should be UTF-8")
            .trim()
            .to_string()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OciRequest {
    method: String,
    path: String,
    accept: Option<String>,
    authorization_present: bool,
}

struct TlsOciServer {
    authority: String,
    ca_path: PathBuf,
    wrong_ca_path: PathBuf,
    digest: String,
    requests: Arc<Mutex<Vec<OciRequest>>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TlsOciServer {
    fn start(fx: &SmokeFixture) -> Self {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let ca = test_ca();
        let wrong_ca = test_ca();
        let leaf_key = KeyPair::generate().expect("failed to generate TLS fixture leaf key");
        let leaf = CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])
            .expect("failed to create TLS fixture leaf parameters")
            .signed_by(&leaf_key, &ca)
            .expect("failed to sign TLS fixture leaf");

        // Only public CA material is persisted for registryCAs. The ephemeral leaf
        // certificate and private key remain in memory for the lifetime of the server.
        let ca_path = fx.fx.dir.path().join(".devcontainer/fixture-ca.pem");
        let wrong_ca_path = fx.fx.dir.path().join(".devcontainer/fixture-wrong-ca.pem");
        std::fs::write(&ca_path, ca.pem()).expect("failed to write TLS fixture CA");
        std::fs::write(&wrong_ca_path, wrong_ca.pem())
            .expect("failed to write TLS fixture wrong CA");

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![leaf.der().clone()],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der())),
            )
            .expect("failed to build TLS fixture server configuration");
        let listener =
            TcpListener::bind("127.0.0.1:0").expect("failed to bind loopback TLS OCI fixture");
        listener
            .set_nonblocking(true)
            .expect("failed to make TLS OCI listener nonblocking");
        let port = listener
            .local_addr()
            .expect("failed to read TLS OCI listener address")
            .port();

        let blob = minimal_feature_tar();
        let digest = format!("sha256:{:x}", Sha256::digest(&blob));
        let manifest = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.manifest.v1+json",
            "layers": [{
                "mediaType": "application/vnd.devcontainers.layer.v1+tar",
                "digest": digest,
                "size": blob.len()
            }]
        }))
        .expect("failed to serialize TLS OCI fixture manifest");

        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_requests = Arc::clone(&requests);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let server_digest = digest.clone();
        let thread = std::thread::spawn(move || {
            serve_tls_oci(
                listener,
                Arc::new(config),
                &manifest,
                &blob,
                &server_digest,
                server_requests,
                server_shutdown,
            );
        });

        Self {
            authority: format!("localhost:{port}"),
            ca_path,
            wrong_ca_path,
            digest,
            requests,
            shutdown,
            thread: Some(thread),
        }
    }

    fn port(&self) -> u16 {
        self.authority
            .rsplit_once(':')
            .expect("TLS OCI authority should contain a port")
            .1
            .parse()
            .expect("TLS OCI port should be numeric")
    }

    fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("TLS OCI fixture thread panicked");
        }
    }

    fn recorded_requests(&self) -> Vec<OciRequest> {
        self.requests
            .lock()
            .expect("TLS OCI request log mutex was poisoned")
            .clone()
    }
}

impl Drop for TlsOciServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct ExactDockerResources {
    containers: Vec<String>,
    images: Vec<String>,
    cleaned: bool,
}

impl ExactDockerResources {
    fn new(containers: Vec<String>, images: Vec<String>) -> Self {
        Self {
            containers,
            images,
            cleaned: false,
        }
    }

    fn cleanup_and_assert_absent(&mut self) {
        cleanup_exact_resources(&self.containers, &self.images, true);
        self.cleaned = true;
    }
}

impl Drop for ExactDockerResources {
    fn drop(&mut self) {
        if !self.cleaned {
            cleanup_exact_resources(&self.containers, &self.images, false);
        }
    }
}

fn test_ca() -> CertifiedIssuer<'static, KeyPair> {
    let mut params =
        CertificateParams::new(Vec::<String>::new()).expect("failed to create CA parameters");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    CertifiedIssuer::self_signed(
        params,
        KeyPair::generate().expect("failed to generate TLS fixture CA key"),
    )
    .expect("failed to create TLS fixture CA")
}

fn minimal_feature_tar() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut archive = tar::Builder::new(&mut bytes);

    let mut root = tar::Header::new_gnu();
    root.as_mut_bytes()[..2].copy_from_slice(b"./");
    root.set_size(0);
    root.set_mode(0o755);
    root.set_entry_type(tar::EntryType::Directory);
    root.set_cksum();
    archive
        .append(&root, std::io::empty())
        .expect("failed to append explicit archive root");

    append_tar_file(
        &mut archive,
        "devcontainer-feature.json",
        br#"{"id":"tls-smoke-feature","version":"1.0.0","name":"TLS smoke Feature"}"#,
        0o644,
    );
    append_tar_file(
        &mut archive,
        "install.sh",
        format!(
            "#!/bin/sh\nset -eu\nmkdir -p /usr/local/share/dcc-tls-feature\nprintf '%s\\n' '{FEATURE_MARKER}' > /usr/local/share/dcc-tls-feature/marker\n"
        )
        .as_bytes(),
        0o755,
    );
    archive.finish().expect("failed to finish Feature archive");
    drop(archive);
    bytes
}

fn append_tar_file(
    archive: &mut tar::Builder<&mut Vec<u8>>,
    path: &str,
    contents: &[u8],
    mode: u32,
) {
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(mode);
    header.set_entry_type(tar::EntryType::Regular);
    header.set_cksum();
    archive
        .append_data(&mut header, path, contents)
        .unwrap_or_else(|error| panic!("failed to append {path} to Feature archive: {error}"));
}

fn serve_tls_oci(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
    manifest: &[u8],
    blob: &[u8],
    digest: &str,
    requests: Arc<Mutex<Vec<OciRequest>>>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Acquire) {
        let (stream, _) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) => panic!("TLS OCI fixture accept failed: {error}"),
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("failed to set TLS OCI read timeout");
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .expect("failed to set TLS OCI write timeout");
        let connection =
            ServerConnection::new(Arc::clone(&config)).expect("failed to create TLS connection");
        let mut tls = StreamOwned::new(connection, stream);
        let Some(request) = read_oci_request(&mut tls) else {
            continue;
        };
        let path = request.path.clone();
        let (status, content_type, body) = if path == "/v2/" {
            ("200 OK", "application/json", b"{}".as_slice())
        } else if path == format!("/v2/{FEATURE_REPOSITORY}/manifests/{FEATURE_TAG}") {
            (
                "200 OK",
                "application/vnd.oci.image.manifest.v1+json",
                manifest,
            )
        } else if path == format!("/v2/{FEATURE_REPOSITORY}/blobs/{digest}") {
            ("200 OK", "application/octet-stream", blob)
        } else {
            ("404 Not Found", "application/json", b"{}".as_slice())
        };
        {
            let mut requests = requests
                .lock()
                .expect("TLS OCI request log mutex was poisoned");
            assert!(
                requests.len() < MAX_RECORDED_REQUESTS,
                "TLS OCI fixture received too many requests"
            );
            requests.push(request);
        }
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        tls.write_all(response.as_bytes())
            .expect("failed to write TLS OCI response headers");
        tls.write_all(body)
            .expect("failed to write TLS OCI response body");
        tls.flush().expect("failed to flush TLS OCI response");
    }
}

fn read_oci_request(stream: &mut impl std::io::Read) -> Option<OciRequest> {
    const MAX_REQUEST_HEAD: usize = 32 * 1024;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 2048];
    loop {
        let read = match stream.read(&mut chunk) {
            Ok(0) => return None,
            Ok(read) => read,
            Err(_) => return None,
        };
        bytes.extend_from_slice(&chunk[..read]);
        assert!(
            bytes.len() <= MAX_REQUEST_HEAD,
            "TLS OCI request headers exceeded {MAX_REQUEST_HEAD} bytes"
        );
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let head = String::from_utf8(bytes).expect("TLS OCI request headers were not UTF-8");
    let mut lines = head.split("\r\n");
    let request_line = lines.next().expect("TLS OCI request line was missing");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("TLS OCI request method was missing");
    let path = parts.next().expect("TLS OCI request path was missing");
    assert_eq!(parts.next(), Some("HTTP/1.1"));
    assert!(
        parts.next().is_none(),
        "TLS OCI request line had extra data"
    );
    let mut accept = None;
    let mut authorization_present = false;
    for line in lines.take_while(|line| !line.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            panic!("TLS OCI request contained a malformed header");
        };
        if name.eq_ignore_ascii_case("accept") {
            accept = Some(value.trim().to_string());
        } else if name.eq_ignore_ascii_case("authorization") {
            authorization_present = true;
        }
    }
    Some(OciRequest {
        method: method.to_string(),
        path: path.to_string(),
        accept,
        authorization_present,
    })
}

fn write_feature_config(fx: &SmokeFixture, server: &TlsOciServer, ca_path: Option<&Path>) {
    let trust = ca_path.map_or_else(String::new, |path| {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("TLS fixture CA path should have a UTF-8 file name");
        format!(
            r#",
            "customizations": {{
                "dcc": {{
                    "registryCAs": {{
                        "{}": "{}"
                    }}
                }}
            }}"#,
            server.authority, file_name
        )
    });
    fx.write_config(&format!(
        r#"{{
            "image": "{IMAGE}",
            "containerUser": "root",
            "features": {{
                "{}/{FEATURE_REPOSITORY}:{FEATURE_TAG}": {{}}
            }}{}
        }}"#,
        server.authority, trust
    ));
}

fn write_executable_from_child(path: &Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    let mut writer = Command::new("/bin/sh")
        .args(["-c", "/bin/cat >\"$1\"", "dcc-tls-smoke-writer"])
        .arg(path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("failed to spawn fake Docker writer");
    writer
        .stdin
        .take()
        .expect("fake Docker writer stdin was missing")
        .write_all(contents.as_bytes())
        .expect("failed to write fake Docker executable");
    assert!(writer.wait().expect("failed to wait for writer").success());
    let mut permissions = std::fs::metadata(path)
        .expect("failed to stat fake Docker executable")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
        .expect("failed to make fake Docker executable runnable");
}

fn dcc_with_failing_docker(fx: &SmokeFixture, marker: &Path) -> Output {
    let bin = fx.fx.dir.path().join("fake-docker-bin");
    std::fs::create_dir_all(&bin).expect("failed to create fake Docker directory");
    write_executable_from_child(
        &bin.join("docker"),
        "#!/bin/sh\nset -eu\n: > \"$DCC_FAKE_DOCKER_MARKER\"\nexit 97\n",
    );
    let mut paths = vec![bin];
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    let path = std::env::join_paths(paths).expect("failed to construct fake Docker PATH");
    fx.fx
        .dcc(&["build"])
        .env("PATH", path)
        .env("DCC_FAKE_DOCKER_MARKER", marker)
        .output()
        .expect("failed to run dcc with failing Docker")
}

fn docker(args: &[&str]) -> Output {
    Command::new("docker")
        .args(args)
        .output()
        .expect("failed to run docker")
}

fn assert_oci_request_cycle(requests: &[OciRequest], digest: &str) {
    assert_eq!(
        requests.len(),
        3,
        "unexpected OCI request cycle: {requests:#?}"
    );
    assert!(requests.iter().all(|request| request.method == "GET"));
    assert_eq!(requests[0].path, "/v2/");
    assert_eq!(
        requests[1].path,
        format!("/v2/{FEATURE_REPOSITORY}/manifests/{FEATURE_TAG}")
    );
    assert_eq!(
        requests[1].accept.as_deref(),
        Some("application/vnd.oci.image.manifest.v1+json")
    );
    assert_eq!(
        requests[2].path,
        format!("/v2/{FEATURE_REPOSITORY}/blobs/{digest}")
    );
    assert!(
        requests
            .iter()
            .all(|request| !request.authorization_present),
        "the unauthenticated fixture received an Authorization header: {requests:#?}"
    );
}

fn assert_tls_trust_failure(output: &Output, authority: &str) {
    assert_failure(output);
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    assert!(
        stderr.contains(&authority.to_ascii_lowercase()),
        "TLS failure did not identify the fixture authority: {stderr}"
    );
    assert!(
        stderr.contains("certificate")
            || stderr.contains("unknownissuer")
            || stderr.contains("unknown issuer"),
        "failure did not preserve certificate-verification context: {stderr}"
    );
}

fn cleanup_exact_resources(containers: &[String], images: &[String], checked: bool) {
    for container in containers.iter().rev() {
        let inspect = Command::new("docker")
            .args(["container", "inspect", container])
            .output();
        if inspect.as_ref().is_ok_and(|output| output.status.success()) {
            let removed = docker(&["container", "rm", "-f", container]);
            if checked {
                assert_success(&removed);
            }
        }
    }
    for image in images.iter().rev() {
        let inspect = Command::new("docker")
            .args(["image", "inspect", image])
            .output();
        if inspect.as_ref().is_ok_and(|output| output.status.success()) {
            let removed = docker(&["image", "rm", "-f", image]);
            if checked {
                assert_success(&removed);
            }
        }
    }
    if checked {
        for container in containers {
            assert_docker_object_absent("container", container);
        }
        for image in images {
            assert_docker_object_absent("image", image);
        }
    }
}

fn assert_docker_object_absent(kind: &str, name: &str) {
    let output = docker(&[kind, "inspect", name]);
    assert_failure(&output);
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    assert!(
        stderr.contains("no such") || stderr.contains("not found"),
        "Docker {kind} inspect failed for a reason other than absence: {stderr}"
    );
}

fn exercise_forced_failure_cleanup(image: &str, container_id: &str) {
    let probe_image = format!("{container_id}-tls-cleanup-probe");
    let probe_container = format!("{container_id}-tls-cleanup-probe");
    let result = create_probe_then_fail(image, container_id, &probe_container, &probe_image);
    assert_eq!(result, Err("forced cleanup probe failure"));
    assert_docker_object_absent("container", &probe_container);
    assert_docker_object_absent("image", &probe_image);
}

fn create_probe_then_fail(
    image: &str,
    container_id: &str,
    probe_container: &str,
    probe_image: &str,
) -> Result<(), &'static str> {
    let _resources = ExactDockerResources::new(
        vec![probe_container.to_string()],
        vec![probe_image.to_string()],
    );
    assert_success(&docker(&["image", "tag", image, probe_image]));
    assert_success(&docker(&[
        "container",
        "create",
        "--name",
        probe_container,
        "--label",
        &format!("dcc.tls_oci_smoke={container_id}"),
        probe_image,
        "/bin/true",
    ]));
    Err("forced cleanup probe failure")
}

fn stop_server_and_assert_cleanup(server: &mut TlsOciServer) {
    let port = server.port();
    server.stop();
    assert!(
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}")
                .parse()
                .expect("failed to parse stopped TLS OCI address"),
            Duration::from_millis(250),
        )
        .is_err(),
        "TLS OCI listener remained reachable after cleanup"
    );
}

#[test]
fn smoke_package_has_explicit_root_metadata_and_executable_installer() {
    let blob = minimal_feature_tar();
    assert_eq!(&blob[..2], b"./", "first tar entry must be explicit ./");
    let mut entries = tar::Archive::new(std::io::Cursor::new(blob))
        .entries()
        .expect("failed to read smoke Feature archive")
        .map(|entry| {
            let mut entry = entry.expect("failed to read smoke Feature entry");
            let mut contents = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut contents)
                .expect("failed to read smoke Feature entry contents");
            (
                entry
                    .path()
                    .expect("failed to read smoke Feature path")
                    .to_string_lossy()
                    .into_owned(),
                entry.header().entry_type(),
                entry.header().mode().expect("failed to read entry mode"),
                contents,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(entries.remove(0).0, "./");
    let metadata = entries
        .iter()
        .find(|(path, kind, _, _)| path == "devcontainer-feature.json" && kind.is_file())
        .expect("Feature metadata entry was missing");
    let metadata_json: serde_json::Value =
        serde_json::from_slice(&metadata.3).expect("Feature metadata was not valid JSON");
    assert_eq!(metadata_json["id"], "tls-smoke-feature");
    assert_eq!(metadata_json["version"], "1.0.0");
    let installer = entries
        .iter()
        .find(|(path, kind, mode, _)| path == "install.sh" && kind.is_file() && mode & 0o111 != 0)
        .expect("executable Feature installer was missing");
    assert!(String::from_utf8_lossy(&installer.3).contains(FEATURE_MARKER));
}

#[test]
#[ignore]
fn missing_and_wrong_ca_fail_before_docker_build() {
    let fx = SmokeFixture::new();
    let root = fx.fx.dir.path().to_path_buf();
    let mut server = TlsOciServer::start(&fx);
    let fake_docker_marker = root.join("fake-docker-called");

    write_feature_config(&fx, &server, None);
    let missing_ca = dcc_with_failing_docker(&fx, &fake_docker_marker);
    assert_tls_trust_failure(&missing_ca, &server.authority);
    assert!(
        !fake_docker_marker.exists(),
        "missing CA reached Docker instead of failing at TLS"
    );
    assert!(server.recorded_requests().is_empty());

    write_feature_config(&fx, &server, Some(&server.wrong_ca_path));
    let wrong_ca = dcc_with_failing_docker(&fx, &fake_docker_marker);
    assert_tls_trust_failure(&wrong_ca, &server.authority);
    assert!(
        !fake_docker_marker.exists(),
        "wrong CA reached Docker instead of failing at TLS"
    );
    assert!(server.recorded_requests().is_empty());

    stop_server_and_assert_cleanup(&mut server);
    drop(server);
    drop(fx);
    assert!(
        !root.exists(),
        "failure path left CA material or workspace content behind"
    );
}

#[test]
#[ignore]
fn trusted_tls_oci_feature_build_runs_marker_and_cleans_exact_resources() {
    let fx = SmokeFixture::new();
    let root = fx.fx.dir.path().to_path_buf();
    let mut server = TlsOciServer::start(&fx);
    write_feature_config(&fx, &server, Some(&server.ca_path));

    let image = fx.container_id();
    let marker_container = format!("{image}-tls-marker");
    let mut resources = ExactDockerResources::new(
        vec![
            image.clone(),
            format!("{image}-build-prep"),
            marker_container.clone(),
        ],
        vec![image.clone(), format!("{image}-base")],
    );
    assert_success(&fx.dcc(&["build"]));
    assert_oci_request_cycle(&server.recorded_requests(), &server.digest);

    let marker_check =
        format!("test \"$(cat /usr/local/share/dcc-tls-feature/marker)\" = '{FEATURE_MARKER}'");
    assert_success(&docker(&[
        "run",
        "--rm",
        "--name",
        &marker_container,
        "--label",
        &format!("dcc.tls_oci_smoke={image}"),
        &image,
        "/bin/sh",
        "-c",
        &marker_check,
    ]));
    exercise_forced_failure_cleanup(&image, &image);

    resources.cleanup_and_assert_absent();
    stop_server_and_assert_cleanup(&mut server);
    drop(server);
    drop(fx);
    assert!(
        !root.exists(),
        "ephemeral CA material and workspace survived cleanup"
    );
}
