//! Local process/socket tests only; these never invoke Docker.
#![cfg(target_os = "linux")]
use std::{
    fs,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct Relay {
    temp: tempfile::TempDir,
    child: Child,
    proxy: u16,
}
impl Relay {
    fn new(target: u16, proxy: u16) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("state/relay/mapping")).unwrap();
        for (name, source) in [
            ("dcc-relay", include_str!("../src/relay.sh")),
            ("dcc-relay-service", include_str!("../src/relay_service.sh")),
        ] {
            let path = root.join(name);
            fs::write(
                &path,
                source
                    .replace(
                        "SHARE=/usr/local/share/dcc",
                        &format!("SHARE='{}'", root.display()),
                    )
                    .replace(
                        "STATE=/run/dcc",
                        &format!("STATE='{}'", root.join("state").display()),
                    ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let child = Command::new(root.join("dcc-relay-service"))
            .arg(root.join("state/relay/mapping"))
            .arg(proxy.to_string())
            .arg(target.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self { temp, child, proxy }
    }
    fn path(&self, path: &str) -> PathBuf {
        self.temp.path().join("state/relay/mapping").join(path)
    }
    fn wait_state(&mut self, state: &str) {
        let end = Instant::now() + Duration::from_secs(12);
        loop {
            let status = fs::read_to_string(self.path("status")).unwrap_or_default();
            if status.starts_with(state) {
                return;
            }
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "relay exited; status={status}"
            );
            assert!(
                Instant::now() < end,
                "timed out waiting for {state}, status={status}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
impl Drop for Relay {
    fn drop(&mut self) {
        fs::write(self.temp.path().join("state/relay/shutdown"), "").unwrap();
        let end = Instant::now() + Duration::from_secs(8);
        while self.child.try_wait().unwrap().is_none() {
            if Instant::now() > end {
                let _ = self.child.kill();
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.wait();
    }
}
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

#[test]
#[ignore = "requires local socat, setsid and Linux /proc; no Docker"]
fn relay_readiness_half_close_restart_and_descendant_cleanup() {
    assert!(Command::new("socat")
        .arg("-V")
        .stdout(Stdio::null())
        .status()
        .unwrap()
        .success());
    let target = free_port();
    let proxy = free_port();
    let mut relay = Relay::new(target, proxy);
    relay.wait_state("ready"); // no application exists yet
    let app = TcpListener::bind(("127.0.0.1", target)).unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = app.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(6)))
            .unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"request");
        stream.write_all(b"response after EOF").unwrap();
    });
    let mut client = TcpStream::connect(("127.0.0.1", relay.proxy)).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(6)))
        .unwrap();
    client.write_all(b"request").unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut bytes = Vec::new();
    client.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"response after EOF");
    server.join().unwrap();
    // Kill only the listener; its still-live process group must be cleaned before restart.
    let listener = fs::read_to_string(relay.path("generation-0/listener")).unwrap();
    assert!(Command::new("kill")
        .args(["-KILL", listener.trim()])
        .status()
        .unwrap()
        .success());
    relay.wait_state("recovering");
    relay.wait_state("ready");
    assert!(relay.path("generation-1/listener").exists());
    // Keep a live connection across shutdown, then verify listener + workers release it.
    let app = TcpListener::bind(("127.0.0.1", target)).unwrap();
    let mut client = TcpStream::connect(("127.0.0.1", proxy)).unwrap();
    let (_backend, _) = app.accept().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(8)))
        .unwrap();
    drop(relay);
    assert_eq!(client.read(&mut [0]).unwrap(), 0);
    TcpListener::bind(("0.0.0.0", proxy)).unwrap();
}

#[test]
#[ignore = "requires local socat, setsid and Linux /proc; no Docker"]
fn foreign_listener_is_not_mistaken_for_relay_readiness() {
    let foreign = TcpListener::bind("0.0.0.0:0").unwrap();
    let mut relay = Relay::new(free_port(), foreign.local_addr().unwrap().port());
    // Failed status is terminal, so read it without requiring the runner to remain alive.
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if fs::read_to_string(relay.path("status"))
            .unwrap_or_default()
            .starts_with("failed")
        {
            break;
        }
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(50));
    }
    relay.child.wait().unwrap();
    assert!(TcpStream::connect(foreign.local_addr().unwrap()).is_ok());
    assert!(!relay.path("generation-1").exists());
}

#[test]
#[ignore = "requires local socat, setsid and Linux /proc; no Docker"]
fn listener_restart_budget_is_lifetime_bounded() {
    let mut relay = Relay::new(free_port(), free_port());
    relay.wait_state("ready");
    for attempt in 0..=3 {
        let listener =
            fs::read_to_string(relay.path(&format!("generation-{attempt}/listener"))).unwrap();
        assert!(Command::new("kill")
            .args(["-KILL", listener.trim()])
            .status()
            .unwrap()
            .success());
        relay.wait_state("recovering");
        if attempt < 3 {
            relay.wait_state("ready");
        }
    }
    let end = Instant::now() + Duration::from_secs(6);
    while !fs::read_to_string(relay.path("status"))
        .unwrap_or_default()
        .starts_with("degraded")
    {
        assert!(Instant::now() < end);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!relay.path("generation-4").exists());
}

#[test]
#[ignore = "requires local socat, setsid and Linux /proc; no Docker"]
fn socat_closing_wait_does_not_limit_fully_open_idle_connections() {
    let app = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut relay = Relay::new(app.local_addr().unwrap().port(), free_port());
    relay.wait_state("ready");
    let mut client = TcpStream::connect(("127.0.0.1", relay.proxy)).unwrap();
    let (mut backend, _) = app.accept().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(6)))
        .unwrap();
    std::thread::sleep(Duration::from_millis(2300));
    backend.write_all(b"still open").unwrap();
    let mut bytes = [0; 10];
    client.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"still open");
    client.shutdown(Shutdown::Write).unwrap();
    let start = Instant::now();
    assert_eq!(client.read(&mut [0]).unwrap(), 0);
    assert!(start.elapsed() >= Duration::from_millis(1500));
    assert!(start.elapsed() < Duration::from_secs(5));
}
