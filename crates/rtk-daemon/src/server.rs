use crate::lifecycle::GracefulShutdown;
use crate::protocol::{handle_request, Request};
use anyhow::{Context as AnyhowContext, Result};
use rtk_core::config::Config;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tokio::time::{error::Elapsed, timeout};
use tracing::{error, info, warn};

#[cfg(unix)]
use std::path::Path;
#[cfg(unix)]
use tokio::net::UnixListener;

#[cfg(windows)]
use tokio::net::TcpListener;

use futures::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio_util::codec::{Framed, LinesCodec};

/// Global connection counter for enforcing max_connections limit
static CONNECTION_COUNT: AtomicU32 = AtomicU32::new(0);

/// RAII guard that decrements connection counter when dropped
/// Prevents counter leaks even if the connection handler panics
struct ConnectionGuard;

impl ConnectionGuard {
    /// Attempt to increment connection count atomically
    /// Returns Some(guard) if under limit, None if at limit
    fn try_acquire(max_connections: u32) -> Option<Self> {
        loop {
            let current = CONNECTION_COUNT.load(Ordering::Acquire);
            if current >= max_connections {
                return None;
            }
            // Atomically increment only if value hasn't changed
            match CONNECTION_COUNT.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Self),
                Err(_) => continue, // Value changed, retry
            }
        }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        CONNECTION_COUNT.fetch_sub(1, Ordering::Release);
    }
}

pub async fn run(_socket_path: String, config: Config) -> Result<()> {
    #[cfg(unix)]
    {
        run_unix(_socket_path, config).await
    }

    #[cfg(windows)]
    {
        // On Windows, use TCP instead of Unix socket
        // Read address from config or use default
        let addr = config
            .daemon
            .tcp_address
            .clone()
            .unwrap_or_else(|| "127.0.0.1:9876".to_string());
        run_tcp(addr, config).await
    }
}

#[cfg(unix)]
async fn run_unix(socket_path: String, config: Config) -> Result<()> {
    // Remove stale socket
    if Path::new(&socket_path).exists() {
        std::fs::remove_file(&socket_path)
            .with_context(|| format!("Failed to remove stale socket: {}", socket_path))?;
    }

    // Bind to Unix socket
    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("Failed to bind to socket: {}", socket_path))?;

    let max_connections = config.daemon.max_connections;
    info!(
        "Daemon listening on {} (max connections: {})",
        socket_path, max_connections
    );

    // Setup graceful shutdown
    let shutdown = GracefulShutdown::new();

    loop {
        tokio::select! {
            // Accept new connections
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, addr)) => {
                        // Atomically check and increment connection limit
                        let guard = match ConnectionGuard::try_acquire(max_connections) {
                            Some(g) => g,
                            None => {
                                let current = CONNECTION_COUNT.load(Ordering::Acquire);
                                warn!("Connection limit reached ({}), rejecting connection from {:?}", max_connections, addr);
                                // Drop the stream to close it
                                drop(stream);
                                continue;
                            }
                        };

                        let current = CONNECTION_COUNT.load(Ordering::Acquire);
                        info!("New connection from {:?} (active: {})", addr, current);

                        let config = config.clone();
                        tokio::spawn(async move {
                            // Guard will be dropped at end of scope, decrementing counter
                            let _guard = guard;
                            if let Err(e) = handle_connection_unix(stream, config).await {
                                if !is_expected_close(&e) {
                                    error!("Connection error: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }

            // Handle shutdown signal
            result = shutdown.wait() => {
                if let Err(e) = result {
                    error!("Shutdown handler error: {}", e);
                }
                info!("Shutdown signal received");
                break;
            }
        }
    }

    // Cleanup
    if Path::new(&socket_path).exists() {
        std::fs::remove_file(&socket_path)?;
    }

    info!("Daemon stopped");
    Ok(())
}

#[cfg(windows)]
async fn run_tcp(addr: String, config: Config) -> Result<()> {
    // Bind to TCP
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind to address: {}", addr))?;

    let max_connections = config.daemon.max_connections;
    info!(
        "Daemon listening on {} (max connections: {})",
        addr, max_connections
    );

    // Setup graceful shutdown
    let shutdown = GracefulShutdown::new();

    loop {
        tokio::select! {
            // Accept new connections
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, peer_addr)) => {
                        // Atomically check and increment connection limit
                        let guard = match ConnectionGuard::try_acquire(max_connections) {
                            Some(g) => g,
                            None => {
                                warn!("Connection limit reached ({}), rejecting connection from {:?}", max_connections, peer_addr);
                                // Drop the stream to close it
                                drop(stream);
                                continue;
                            }
                        };

                        let current = CONNECTION_COUNT.load(Ordering::Acquire);
                        info!("New connection from {:?} (active: {})", peer_addr, current);

                        let config = config.clone();
                        tokio::spawn(async move {
                            // Guard will be dropped at end of scope, decrementing counter
                            let _guard = guard;
                            if let Err(e) = handle_connection_tcp(stream, config).await {
                                if !is_expected_close(&e) {
                                    error!("Connection error: {}", e);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }

            // Handle shutdown signal
            result = shutdown.wait() => {
                if let Err(e) = result {
                    error!("Shutdown handler error: {}", e);
                }
                info!("Shutdown signal received");
                break;
            }
        }
    }

    info!("Daemon stopped");
    Ok(())
}

/// Check if error is an expected close (client disconnect, timeout)
fn is_expected_close(error: &anyhow::Error) -> bool {
    // Check for timeout
    if error.downcast_ref::<Elapsed>().is_some() {
        return true;
    }

    // Check for IO errors that indicate clean disconnect
    if let Some(io_err) = error.downcast_ref::<std::io::Error>() {
        matches!(
            io_err.kind(),
            std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe
        )
    } else {
        false
    }
}

#[cfg(unix)]
async fn handle_connection_unix(stream: tokio::net::UnixStream, config: Config) -> Result<()> {
    let timeout_duration = Duration::from_secs(config.daemon.timeout_seconds);

    // Use LinesCodec for newline-delimited JSON (NDJSON) framing
    // This handles partial messages and message boundaries correctly
    let mut framed = Framed::new(stream, LinesCodec::new());

    loop {
        // Apply timeout to each read operation
        let line_result = timeout(timeout_duration, framed.next()).await;

        match line_result {
            // Timeout occurred
            Err(_) => {
                warn!(
                    "Connection timed out after {} seconds",
                    timeout_duration.as_secs()
                );
                return Err(anyhow::anyhow!("Connection timeout"));
            }

            // Read completed (may be error or None)
            Ok(None) => {
                info!("Connection closed by client");
                break;
            }
            Ok(Some(Err(e))) => {
                warn!("Failed to read line: {}", e);
                return Err(e).context("Stream read error");
            }
            Ok(Some(Ok(line))) => {
                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Parse JSON request
                let request: Request = match serde_json::from_str(&line) {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("Failed to parse request: {}", e);
                        let error_response =
                            crate::protocol::error_response(None, -32700, "Parse error");
                        // Get the inner stream for writing error response
                        if let Err(write_err) =
                            write_with_timeout(framed.get_mut(), &error_response, timeout_duration)
                                .await
                        {
                            error!("Failed to write error response: {}", write_err);
                            return Err(write_err);
                        }
                        continue;
                    }
                };

                // Handle request and get response
                let response = handle_request(request, &config).await;

                // Write response with timeout
                if let Err(e) =
                    write_with_timeout(framed.get_mut(), &response, timeout_duration).await
                {
                    error!("Failed to write response: {}", e);
                    return Err(e);
                }
            }
        }
    }

    Ok(())
}

#[cfg(windows)]
async fn handle_connection_tcp(stream: tokio::net::TcpStream, config: Config) -> Result<()> {
    let timeout_duration = Duration::from_secs(config.daemon.timeout_seconds);

    // Use LinesCodec for newline-delimited JSON (NDJSON) framing
    // This handles partial messages and message boundaries correctly
    let mut framed = Framed::new(stream, LinesCodec::new());

    loop {
        // Apply timeout to each read operation
        let line_result = timeout(timeout_duration, framed.next()).await;

        match line_result {
            // Timeout occurred
            Err(_) => {
                warn!(
                    "Connection timed out after {} seconds",
                    timeout_duration.as_secs()
                );
                return Err(anyhow::anyhow!("Connection timeout"));
            }

            // Read completed (may be error or None)
            Ok(None) => {
                info!("Connection closed by client");
                break;
            }
            Ok(Some(Err(e))) => {
                warn!("Failed to read line: {}", e);
                return Err(e).context("Stream read error");
            }
            Ok(Some(Ok(line))) => {
                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Parse JSON request
                let request: Request = match serde_json::from_str(&line) {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("Failed to parse request: {}", e);
                        let error_response =
                            crate::protocol::error_response(None, -32700, "Parse error");
                        // Get the inner stream for writing error response
                        if let Err(write_err) =
                            write_with_timeout(framed.get_mut(), &error_response, timeout_duration)
                                .await
                        {
                            error!("Failed to write error response: {}", write_err);
                            return Err(write_err);
                        }
                        continue;
                    }
                };

                // Handle request and get response
                let response = handle_request(request, &config).await;

                // Write response with timeout
                if let Err(e) =
                    write_with_timeout(framed.get_mut(), &response, timeout_duration).await
                {
                    error!("Failed to write response: {}", e);
                    return Err(e);
                }
            }
        }
    }

    Ok(())
}

/// Write data to stream with timeout
/// Adds newline delimiter for NDJSON protocol
async fn write_with_timeout<S>(
    stream: &mut S,
    data: &[u8],
    timeout_duration: Duration,
) -> Result<()>
where
    S: AsyncWriteExt + Unpin,
{
    timeout(timeout_duration, async {
        stream.write_all(data).await?;
        // Add newline delimiter for NDJSON protocol
        stream.write_all(b"\n").await?;
        stream.flush().await?;
        Ok(())
    })
    .await
    .context("Write operation timed out")?
}
