use std::process::Stdio;

use anyhow::Context as _;
use tokio::{
    io,
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

/// Binds a host-side TCP listener on `127.0.0.1` for each port and spawns a
/// relay task. Returns the task handles so the caller can abort them on exit.
pub(crate) async fn forward_ports(
    container: &str,
    ports: &[u16],
) -> anyhow::Result<Vec<JoinHandle<()>>> {
    let mut handles = Vec::with_capacity(ports.len() * 2);
    for &port in ports {
        let v4 = TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .with_context(|| format!("failed to bind 127.0.0.1:{port} for forwarding"))?;
        let v6 = TcpListener::bind(format!("[::1]:{port}"))
            .await
            .with_context(|| format!("failed to bind [::1]:{port} for forwarding"))?;
        tracing::info!(port, "forwarding port");
        let container = container.to_owned();
        handles.push(tokio::spawn(relay_port(v4, container.clone(), port)));
        handles.push(tokio::spawn(relay_port(v6, container, port)));
    }
    Ok(handles)
}

/// Accepts connections on `listener` and spawns a per-connection relay task.
///
/// Each relay opens a `docker exec -i <container> nc 127.0.0.1 <port>` subprocess
/// and copies bytes bidirectionally between the accepted TCP socket and that
/// process's stdin/stdout. From the application's perspective the connection
/// arrives on its own loopback interface, not via the Docker bridge.
///
/// Per-connection tasks are spawned without retaining their handles because they
/// are inherently short-lived (they exit when either side closes the connection)
/// and self-cleaning (the subprocess exits when the container is gone). The
/// relay task itself is aborted by the caller via the returned `JoinHandle`.
async fn relay_port(listener: TcpListener, container: String, port: u16) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let container = container.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, &container, port).await {
                        tracing::debug!(port, error = %e, "port relay connection closed");
                    }
                });
            }
            Err(e) => {
                tracing::warn!(port, error = %e, "port relay listener error");
                break;
            }
        }
    }
}

async fn handle_connection(stream: TcpStream, container: &str, port: u16) -> anyhow::Result<()> {
    let mut child = tokio::process::Command::new("docker")
        .args([
            "exec",
            "-i",
            container,
            "nc",
            "localhost",
            &port.to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn `docker exec nc`")?;

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

    tokio::select! {
        _ = io::copy(&mut tcp_rx, &mut proc_stdin) => {}
        _ = io::copy(&mut proc_stdout, &mut tcp_tx) => {}
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Relay logic requires a live Docker daemon and a running container.
    // forward_ports bind errors are exercised indirectly via integration tests.
}
