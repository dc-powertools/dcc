use std::{ffi::OsStr, future::Future, process::Stdio};

use anyhow::Context as _;
use tokio::{
    io::{self, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};

#[cfg(test)]
use tokio::io::{AsyncRead, AsyncWrite};

pub(crate) const CONNECTOR_PATH: &str = "/usr/local/share/dcc/dcc-connect";

pub(crate) fn baked_connector_asset() -> (String, Vec<u8>, u32) {
    (
        ".dcc-generated/dcc-connect".to_string(),
        connector_script().as_bytes().to_vec(),
        0o755,
    )
}

fn connector_script() -> &'static str {
    r#"#!/bin/sh
set -eu

fail() {
    printf '%s\n' "dcc-connect: $1" >&2
    exit "${2:-1}"
}

select_connector() {
    if command -v nc.openbsd >/dev/null 2>&1; then
        printf '%s\n' nc.openbsd
        return 0
    fi

    if command -v ncat >/dev/null 2>&1; then
        ncat_version=$(ncat --version 2>&1 || :)
        case "$ncat_version" in
            *Ncat*)
                printf '%s\n' ncat
                return 0
                ;;
        esac
    fi

    if command -v nc >/dev/null 2>&1; then
        nc_help=$(nc -h 2>&1 || :)
        case " $nc_help " in
            *[[:space:]]-N[[:space:]]*)
                printf '%s\n' nc
                return 0
                ;;
        esac
    fi

    return 1
}

unsupported() {
    fail "no compatible connector found (requires nc.openbsd with -N, Nmap ncat, or nc advertising -N)" 127
}

if [ "$#" -eq 1 ] && [ "$1" = "--check" ]; then
    select_connector >/dev/null || unsupported
    exit 0
fi

[ "$#" -eq 2 ] || fail "usage: dcc-connect HOST PORT" 64
host=$1
port=$2

[ "$host" = "127.0.0.1" ] || fail "HOST must be 127.0.0.1" 64
case "$port" in
    ''|*[!0-9]*) fail "PORT must be an integer from 1 to 65535" 64 ;;
esac
if ! [ "$port" -ge 1 ] 2>/dev/null || ! [ "$port" -le 65535 ] 2>/dev/null; then
    fail "PORT must be an integer from 1 to 65535" 64
fi

connector=$(select_connector) || unsupported
case "$connector" in
    nc.openbsd) exec nc.openbsd -N "$host" "$port" ;;
    ncat) exec ncat "$host" "$port" ;;
    nc) exec nc -N "$host" "$port" ;;
    *) unsupported ;;
esac
"#
}

pub(crate) struct Forwarding {
    tasks: Vec<RelayTask>,
}

struct RelayTask {
    shutdown: Option<oneshot::Sender<()>>,
    handle: Option<JoinHandle<()>>,
}

impl Forwarding {
    /// Stops accepting, aborts every active connector, and waits for their task
    /// futures to be dropped before returning.
    pub(crate) async fn shutdown(mut self) {
        self.request_shutdown();
        for task in &mut self.tasks {
            if let Some(handle) = task.handle.take() {
                let _ = handle.await;
            }
        }
    }

    fn request_shutdown(&mut self) {
        for task in &mut self.tasks {
            if let Some(shutdown) = task.shutdown.take() {
                let _ = shutdown.send(());
            }
        }
    }
}

impl Drop for Forwarding {
    fn drop(&mut self) {
        // If the owning foreground future is cancelled, listener tasks still receive
        // shutdown and perform their own connector cleanup instead of being detached.
        self.request_shutdown();
    }
}

/// Binds host-side TCP listeners for every requested port and starts their relay
/// tasks. IPv4 loopback is required; IPv6 loopback is opportunistic so an IPv4-only
/// host can still use forwarding.
///
/// All listeners are bound before any tasks are spawned. Consequently, a bind error
/// drops every listener acquired by this call and cannot strand a relay task.
pub(crate) async fn forward_ports(container: &str, ports: &[u16]) -> anyhow::Result<Forwarding> {
    let mut listeners = Vec::with_capacity(ports.len() * 2);
    for &port in ports {
        let v4_address = format!("127.0.0.1:{port}");
        let v4 = TcpListener::bind(&v4_address)
            .await
            .with_context(|| format!("failed to bind {v4_address} for forwarding"))?;
        listeners.push((v4, port));

        let v6_address = format!("[::1]:{port}");
        let v6 = TcpListener::bind(&v6_address).await;
        if let Some(v6) = optional_ipv6_listener(v6, &v6_address, port) {
            listeners.push((v6, port));
        }
    }

    let mut handles = Vec::with_capacity(listeners.len());
    for (listener, port) in listeners {
        tracing::info!(port, address = ?listener.local_addr().ok(), "forwarding port");
        let container = container.to_owned();
        handles.push(start_relay_listener(listener, move |stream| {
            let container = container.clone();
            async move {
                if let Err(error) = handle_connection(stream, &container, port).await {
                    tracing::debug!(port, error = %error, "port relay connection closed");
                }
            }
        }));
    }
    Ok(Forwarding { tasks: handles })
}

fn optional_ipv6_listener(
    listener: std::io::Result<TcpListener>,
    address: &str,
    port: u16,
) -> Option<TcpListener> {
    match listener {
        Ok(listener) => Some(listener),
        Err(error) => {
            tracing::warn!(port, %address, %error, "IPv6 loopback unavailable; forwarding on IPv4 only");
            None
        }
    }
}

fn start_relay_listener<H, F>(listener: TcpListener, handler: H) -> RelayTask
where
    H: Fn(TcpStream) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let (shutdown, shutdown_rx) = oneshot::channel();
    let handle = tokio::spawn(relay_listener(listener, handler, shutdown_rx));
    RelayTask {
        shutdown: Some(shutdown),
        handle: Some(handle),
    }
}

/// Accepts connections on `listener` and retains every per-connection task in a
/// `JoinSet`. Shutdown stops acceptance, aborts the set, and joins each cancelled
/// task before the listener future finishes.
async fn relay_listener<H, F>(
    listener: TcpListener,
    handler: H,
    mut shutdown: oneshot::Receiver<()>,
) where
    H: Fn(TcpStream) -> F,
    F: Future<Output = ()> + Send + 'static,
{
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    connections.spawn(handler(stream));
                }
                Err(error) => {
                    tracing::warn!(error = %error, "port relay listener error");
                    break;
                }
            },
            completed = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = completed {
                    tracing::debug!(error = %error, "port relay task failed");
                }
            }
        }
    }

    connections.abort_all();
    while let Some(completed) = connections.join_next().await {
        if let Err(error) = completed {
            if !error.is_cancelled() {
                tracing::debug!(error = %error, "port relay task failed during shutdown");
            }
        }
    }
}

async fn handle_connection(stream: TcpStream, container: &str, port: u16) -> anyhow::Result<()> {
    let command = connector_command("docker", container, port);
    handle_connection_with_command(stream, command).await
}

fn connector_command(
    docker_program: impl AsRef<OsStr>,
    container: &str,
    port: u16,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(docker_program);
    command.args([
        "exec",
        "-i",
        container,
        CONNECTOR_PATH,
        "127.0.0.1",
        &port.to_string(),
    ]);
    command
}

async fn handle_connection_with_command(
    stream: TcpStream,
    mut command: tokio::process::Command,
) -> anyhow::Result<()> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("failed to spawn port-forwarding connector")?;

    let mut proc_stdin = child
        .stdin
        .take()
        // SAFETY: stdin was configured as Stdio::piped() above
        .expect("stdin configured as piped");
    let mut proc_stdout = child
        .stdout
        .take()
        // SAFETY: stdout was configured as Stdio::piped() above
        .expect("stdout configured as piped");
    let (mut tcp_rx, mut tcp_tx) = stream.into_split();

    let relay_result = tokio::try_join!(
        async {
            let copied = io::copy(&mut tcp_rx, &mut proc_stdin).await?;
            proc_stdin.shutdown().await?;
            // ChildStdin::shutdown flushes but does not release the pipe handle.
            // Drop it now so docker exec can observe EOF while stdout is drained.
            drop(proc_stdin);
            Ok::<_, io::Error>(copied)
        },
        async {
            let copied = io::copy(&mut proc_stdout, &mut tcp_tx).await?;
            tcp_tx.shutdown().await?;
            Ok::<_, io::Error>(copied)
        }
    );

    // Reap the connector on success, I/O failure, and cancellation of either copy.
    let _ = child.kill().await;
    let _ = child.wait().await;
    relay_result.context("port-forwarding relay failed")?;
    Ok(())
}

/// Copies until both input directions reach EOF. Each completed input half closes
/// the opposite output half, allowing a request-side half-close to reach the server
/// while the response continues to drain back to the client.
#[cfg(test)]
async fn copy_both_directions<ClientRead, ClientWrite, UpstreamRead, UpstreamWrite>(
    client_read: &mut ClientRead,
    client_write: &mut ClientWrite,
    upstream_read: &mut UpstreamRead,
    upstream_write: &mut UpstreamWrite,
) -> io::Result<(u64, u64)>
where
    ClientRead: AsyncRead + Unpin,
    ClientWrite: AsyncWrite + Unpin,
    UpstreamRead: AsyncRead + Unpin,
    UpstreamWrite: AsyncWrite + Unpin,
{
    tokio::try_join!(
        async {
            let copied = io::copy(client_read, upstream_write).await?;
            upstream_write.shutdown().await?;
            Ok(copied)
        },
        async {
            let copied = io::copy(upstream_read, client_write).await?;
            client_write.shutdown().await?;
            Ok(copied)
        }
    )
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsStr,
        io::{ErrorKind, Write as _},
        net::SocketAddr,
        os::unix::fs::PermissionsExt as _,
        path::Path,
        process::{Command, Output, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver},
            Arc,
        },
        thread,
        time::Duration,
    };

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    use super::*;

    const DEADLINE: Duration = Duration::from_secs(2);

    async fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let client = TcpStream::connect(address);
        let server = listener.accept();
        let (client, server) =
            tokio::time::timeout(DEADLINE, async { tokio::join!(client, server) })
                .await
                .expect("TCP pair timed out");
        let (server, _) = server.unwrap();
        (client.unwrap(), server)
    }

    async fn recv_signal(receiver: &Receiver<()>, description: &str) {
        tokio::time::timeout(DEADLINE, async {
            loop {
                match receiver.try_recv() {
                    Ok(()) => return,
                    Err(mpsc::TryRecvError::Empty) => tokio::task::yield_now().await,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        panic!("{description} sender disconnected")
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {description}"));
    }

    fn write_executable(path: &Path, contents: &str) {
        // Open and write the executable from a single-threaded child. If this
        // multithreaded test process opens it itself, a concurrent fork can inherit
        // the writable descriptor briefly and make the following exec fail with
        // ETXTBSY before CLOEXEC takes effect.
        let mut writer = Command::new("/bin/sh")
            .args(["-c", "/bin/cat >\"$1\"", "dcc-test-writer"])
            .arg(path)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        writer
            .stdin
            .take()
            .unwrap()
            .write_all(contents.as_bytes())
            .unwrap();
        assert!(writer.wait().unwrap().success());

        let mut permissions = std::fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    fn run_connector(programs: &[(&str, &str)], args: &[&str]) -> (Output, String) {
        let temp = tempfile::tempdir().unwrap();
        let connector = temp.path().join("dcc-connect");
        let log = temp.path().join("connector.log");
        write_executable(&connector, connector_script());
        for (name, script) in programs {
            write_executable(&temp.path().join(name), script);
        }

        let output = Command::new(&connector)
            .args(args)
            .env("PATH", temp.path())
            .env("DCC_CONNECT_LOG", &log)
            .output()
            .unwrap();
        let invocation = std::fs::read_to_string(log).unwrap_or_default();
        (output, invocation)
    }

    const RECORDING_CONNECTOR: &str =
        "#!/bin/sh\nprintf '%s\\n' \"${0##*/}|$*\" >\"$DCC_CONNECT_LOG\"\n";
    const NMAP_NCAT: &str = "#!/bin/sh\nif [ \"${1-}\" = --version ]; then printf '%s\\n' 'Ncat: Version 7.95'; exit 0; fi\nprintf '%s\\n' \"${0##*/}|$*\" >\"$DCC_CONNECT_LOG\"\n";
    const GENERIC_WITH_N: &str = "#!/bin/sh\nif [ \"${1-}\" = -h ]; then printf '%s\\n' '  -N shutdown after EOF'; exit 0; fi\nprintf '%s\\n' \"${0##*/}|$*\" >\"$DCC_CONNECT_LOG\"\n";
    const GENERIC_WITHOUT_N: &str = "#!/bin/sh\nif [ \"${1-}\" = -h ]; then printf '%s\\n' 'usage: nc [-46h] host port'; exit 0; fi\nexit 9\n";
    const BUSYBOX_NC: &str = "#!/bin/sh\nif [ \"${1-}\" = -h ]; then printf '%s\\n' 'BusyBox nc: usage: nc [-iNw] HOST PORT'; exit 0; fi\nexit 9\n";
    const TRADITIONAL_NC: &str = "#!/bin/sh\nif [ \"${1-}\" = -h ]; then printf '%s\\n' 'nc [options] hostname port[s] [ports]'; exit 0; fi\nexit 9\n";
    const IMPOSTOR_NCAT: &str = "#!/bin/sh\nif [ \"${1-}\" = --version ]; then printf '%s\\n' 'unrelated connector 1.0'; exit 0; fi\nexit 9\n";

    #[test]
    fn executable_fixture_is_safe_during_parallel_process_spawns() {
        let running = Arc::new(AtomicBool::new(true));
        let churn_running = Arc::clone(&running);
        let churn = thread::spawn(move || {
            while churn_running.load(Ordering::Relaxed) {
                Command::new("/bin/true").status().unwrap();
            }
        });

        let failure = (0..64).find_map(|_| {
            let temp = tempfile::tempdir().unwrap();
            let executable = temp.path().join("fixture");
            write_executable(&executable, "#!/bin/sh\nexit 0\n");
            match Command::new(executable).status() {
                Ok(status) if status.success() => None,
                Ok(status) => Some(format!("fixture exited with {status}")),
                Err(error) => Some(format!("fixture failed to spawn: {error}")),
            }
        });

        running.store(false, Ordering::Relaxed);
        churn.join().unwrap();
        assert!(failure.is_none(), "{}", failure.unwrap_or_default());
    }

    #[test]
    fn docker_connector_command_uses_fixed_baked_boundary() {
        let command = connector_command("fake-docker", "container-name", 8123);
        let command = command.as_std();
        assert_eq!(command.get_program(), OsStr::new("fake-docker"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "exec",
                "-i",
                "container-name",
                CONNECTOR_PATH,
                "127.0.0.1",
                "8123",
            ]
            .map(OsStr::new)
        );
    }

    #[test]
    fn connector_prefers_openbsd_then_nmap_then_compatible_generic_nc() {
        let args = ["127.0.0.1", "8123"];
        let (output, invocation) = run_connector(
            &[
                ("nc.openbsd", RECORDING_CONNECTOR),
                ("ncat", NMAP_NCAT),
                ("nc", GENERIC_WITH_N),
            ],
            &args,
        );
        assert!(output.status.success());
        assert_eq!(invocation, "nc.openbsd|-N 127.0.0.1 8123\n");

        let (output, invocation) =
            run_connector(&[("ncat", NMAP_NCAT), ("nc", GENERIC_WITH_N)], &args);
        assert!(output.status.success());
        assert_eq!(invocation, "ncat|127.0.0.1 8123\n");

        let (output, invocation) = run_connector(&[("nc", GENERIC_WITH_N)], &args);
        assert!(output.status.success());
        assert_eq!(invocation, "nc|-N 127.0.0.1 8123\n");
    }

    #[test]
    fn connector_rejects_arbitrary_nc_and_invalid_arguments() {
        for unsupported in [GENERIC_WITHOUT_N, BUSYBOX_NC, TRADITIONAL_NC] {
            let (output, invocation) =
                run_connector(&[("nc", unsupported)], &["127.0.0.1", "8123"]);
            assert_eq!(output.status.code(), Some(127));
            assert!(invocation.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("no compatible connector"));
        }

        let (output, _) = run_connector(
            &[("ncat", IMPOSTOR_NCAT), ("nc", GENERIC_WITHOUT_N)],
            &["--check"],
        );
        assert_eq!(output.status.code(), Some(127));

        for args in [
            vec!["localhost", "8123"],
            vec!["127.0.0.1", "0"],
            vec!["127.0.0.1", "65536"],
            vec!["127.0.0.1", "not-a-port"],
        ] {
            let (output, _) = run_connector(&[("nc.openbsd", RECORDING_CONNECTOR)], &args);
            assert_eq!(output.status.code(), Some(64), "args: {args:?}");
        }
    }

    #[test]
    fn connector_check_uses_the_runtime_selector_without_connecting() {
        let (output, invocation) = run_connector(&[("ncat", NMAP_NCAT)], &["--check"]);
        assert!(output.status.success());
        assert!(invocation.is_empty());
    }

    #[tokio::test]
    async fn relay_copies_request_and_drains_response_after_client_half_close() {
        let (mut client, server) = tcp_pair().await;
        let (upstream, mut application) = tokio::io::duplex(64);
        let (mut upstream_read, mut upstream_write) = tokio::io::split(upstream);
        let (mut server_read, mut server_write) = server.into_split();

        let relay = tokio::spawn(async move {
            copy_both_directions(
                &mut server_read,
                &mut server_write,
                &mut upstream_read,
                &mut upstream_write,
            )
            .await
        });
        let application_task = tokio::spawn(async move {
            let mut request = Vec::new();
            application.read_to_end(&mut request).await.unwrap();
            assert_eq!(request, b"request");
            application.write_all(b"response-after-eof").await.unwrap();
            application.shutdown().await.unwrap();
        });

        client.write_all(b"request").await.unwrap();
        client.shutdown().await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(DEADLINE, client.read_to_end(&mut response))
            .await
            .expect("response drain timed out")
            .unwrap();
        assert_eq!(response, b"response-after-eof");

        let copied = tokio::time::timeout(DEADLINE, relay)
            .await
            .expect("relay did not finish")
            .unwrap()
            .unwrap();
        assert_eq!(copied, (7, 18));
        application_task.await.unwrap();
    }

    #[tokio::test]
    async fn subprocess_boundary_drains_response_after_client_eof_without_docker() {
        let temp = tempfile::tempdir().unwrap();
        let fake_docker = temp.path().join("docker");
        write_executable(
            &fake_docker,
            r#"#!/bin/sh
[ "$#" -eq 6 ] || exit 20
[ "$1" = exec ] || exit 21
[ "$2" = -i ] || exit 22
[ "$3" = test-container ] || exit 23
[ "$4" = /usr/local/share/dcc/dcc-connect ] || exit 24
[ "$5" = 127.0.0.1 ] || exit 25
[ "$6" = 8123 ] || exit 26
request=$(/bin/cat)
[ "$request" = request ] || exit 27
printf '%s' response-after-eof
"#,
        );

        let (mut client, server) = tcp_pair().await;
        let command = connector_command(&fake_docker, "test-container", 8123);
        let relay = tokio::spawn(handle_connection_with_command(server, command));

        client.write_all(b"request").await.unwrap();
        client.shutdown().await.unwrap();
        let mut response = Vec::new();
        tokio::time::timeout(DEADLINE, client.read_to_end(&mut response))
            .await
            .expect("subprocess response drain timed out")
            .unwrap();
        assert_eq!(response, b"response-after-eof");

        tokio::time::timeout(DEADLINE, relay)
            .await
            .expect("subprocess relay did not finish")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn later_bind_collision_releases_every_listener_without_spawning_tasks() {
        let collision = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let collision_port = collision.local_addr().unwrap().port();
        let available = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let available_port = available.local_addr().unwrap().port();
        drop(available);

        let error = match forward_ports("unused", &[available_port, collision_port]).await {
            Ok(_) => panic!("later bind collision unexpectedly succeeded"),
            Err(error) => error,
        };
        let message = format!("{error:#}");
        assert!(message.contains(&format!("127.0.0.1:{collision_port}")));

        TcpListener::bind(("127.0.0.1", available_port))
            .await
            .expect("earlier listener remained bound after later collision");
    }

    #[test]
    fn unavailable_ipv6_degrades_to_ipv4_only() {
        let error = std::io::Error::new(ErrorKind::AddrNotAvailable, "IPv6 disabled");
        assert!(optional_ipv6_listener(Err(error), "[::1]:8123", 8123).is_none());
    }

    struct DropSignal(Option<mpsc::Sender<()>>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            if let Some(sender) = self.0.take() {
                let _ = sender.send(());
            }
        }
    }

    #[tokio::test]
    async fn shutdown_cancels_and_joins_active_connection_tasks() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let task = start_relay_listener(listener, move |_stream| {
            let started_tx = started_tx.clone();
            let dropped_tx = dropped_tx.clone();
            async move {
                let _drop_signal = DropSignal(Some(dropped_tx));
                started_tx.send(()).unwrap();
                std::future::pending::<()>().await;
            }
        });

        let _client = TcpStream::connect(address).await.unwrap();
        recv_signal(&started_rx, "connection start").await;
        Forwarding { tasks: vec![task] }.shutdown().await;
        recv_signal(&dropped_rx, "connection cancellation").await;
    }

    #[tokio::test]
    async fn connector_spawn_failure_is_reported() {
        let (_client, server) = tcp_pair().await;
        let command =
            tokio::process::Command::new("/definitely/missing/dcc-port-forwarding-connector");
        let error = handle_connection_with_command(server, command)
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("failed to spawn port-forwarding connector"));
    }

    #[tokio::test]
    async fn ipv4_listener_starts_and_accepts_connections() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address: SocketAddr = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = mpsc::channel();
        let task = start_relay_listener(listener, move |_stream| {
            let accepted_tx = accepted_tx.clone();
            async move {
                accepted_tx.send(()).unwrap();
            }
        });

        let _client = TcpStream::connect(address).await.unwrap();
        recv_signal(&accepted_rx, "IPv4 connection acceptance").await;
        Forwarding { tasks: vec![task] }.shutdown().await;
    }
}
