use std::{future::Future, process::Stdio};

use anyhow::Context as _;
use tokio::{
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::{JoinHandle, JoinSet},
};

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
    let mut command = tokio::process::Command::new("docker");
    command.args([
        "exec",
        "-i",
        container,
        "nc",
        "127.0.0.1",
        &port.to_string(),
    ]);
    handle_connection_with_command(stream, command).await
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

    let relay_result =
        copy_both_directions(&mut tcp_rx, &mut tcp_tx, &mut proc_stdout, &mut proc_stdin).await;

    // Reap the connector on success, I/O failure, and cancellation of either copy.
    let _ = child.kill().await;
    let _ = child.wait().await;
    relay_result.context("port-forwarding relay failed")?;
    Ok(())
}

/// Copies until both input directions reach EOF. Each completed input half closes
/// the opposite output half, allowing a request-side half-close to reach the server
/// while the response continues to drain back to the client.
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
        io::ErrorKind,
        net::SocketAddr,
        sync::mpsc::{self, Receiver},
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
