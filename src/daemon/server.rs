use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::daemon::state::DaemonState;
use crate::ipc::{DaemonRequest, OwnedResponse};

struct SocketFileGuard {
    path: Option<PathBuf>,
}

impl SocketFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn cleanup_now(mut self) -> Result<()> {
        if let Some(path) = self.path.take() {
            remove_socket_if_exists(&path)?;
        }
        Ok(())
    }
}

impl Drop for SocketFileGuard {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = remove_socket_if_exists(&path);
        }
    }
}

fn remove_socket_if_exists(socket_path: &Path) -> Result<()> {
    match std::fs::remove_file(socket_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn decode_request_frame(data: &[u8]) -> Result<DaemonRequest> {
    bincode::serde::decode_from_slice::<DaemonRequest, _>(data, bincode::config::standard())
        .map(|(request, _)| request)
        .map_err(Into::into)
}

fn encode_response_frame(response: &OwnedResponse) -> Result<Vec<u8>> {
    bincode::serde::encode_to_vec(response, bincode::config::standard()).map_err(Into::into)
}

fn log_client_error(context: &str, error: &dyn std::fmt::Display) {
    crate::logging::error(&format!("{context}: {error}"));
}

fn decode_request_or_log(data: &[u8]) -> Option<DaemonRequest> {
    decode_request_frame(data).map_or_else(
        |error| {
            log_client_error("Failed to decode client request", &error);
            None
        },
        Some,
    )
}

fn encode_response_or_log(response: &OwnedResponse) -> Option<Vec<u8>> {
    encode_response_frame(response).map_or_else(
        |error| {
            log_client_error("Failed to encode daemon response", &error);
            None
        },
        Some,
    )
}

/// Handles a single connected daemon client until the stream closes or decoding fails.
///
/// # Errors
///
/// Returns an error if reading from or writing to the framed Unix stream fails.
pub async fn handle_client(stream: UnixStream, state: Arc<DaemonState>) -> Result<()> {
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    while let Some(frame) = framed.next().await {
        let data = match frame {
            Ok(data) => data,
            Err(error) => {
                log_client_error("Client stream read error", &error);
                break;
            }
        };

        let Some(request) = decode_request_or_log(&data) else {
            break;
        };

        let response = state.handle_request(request).await;
        let Some(response_data) = encode_response_or_log(&response) else {
            break;
        };

        if let Err(error) = framed.send(response_data.into()).await {
            log_client_error("Client stream write error", &error);
            break;
        }
    }

    Ok(())
}

async fn wait_for_shutdown_signal() -> Result<()> {
    signal::ctrl_c().await?;
    crate::logging::info("\nShutting down daemon...");
    Ok(())
}

async fn accept_loop(listener: &UnixListener, state: Arc<DaemonState>) -> Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, state_clone).await {
                log_client_error("Client handler error", &error);
            }
        });
    }
}

fn ensure_socket_parent_dir(socket_path: &Path) -> Result<()> {
    if let Some(parent_dir) = socket_path.parent() {
        std::fs::create_dir_all(parent_dir)?;
    }
    Ok(())
}

/// Runs the daemon accept loop and serves requests over a Unix domain socket.
///
/// # Errors
///
/// Returns an error if socket cleanup/bind fails, daemon state initialization fails,
/// socket accept fails, or final socket cleanup fails during shutdown.
pub async fn run_daemon(socket_path: PathBuf, foreground: bool) -> Result<()> {
    ensure_socket_parent_dir(&socket_path)?;
    remove_socket_if_exists(&socket_path)?;

    let socket_guard = SocketFileGuard::new(socket_path.clone());
    let listener = UnixListener::bind(&socket_path)?;
    crate::logging::info("VOICEVOX daemon started successfully");
    crate::logging::info(&format!("Listening on: {}", socket_path.display()));

    let state = Arc::new(DaemonState::new()?);

    if !foreground {
        crate::logging::info("Running in background mode. Use Ctrl+C to stop gracefully.");
    }

    tokio::select! {
        result = accept_loop(&listener, Arc::clone(&state)) => result?,
        result = wait_for_shutdown_signal() => result?,
    }

    socket_guard.cleanup_now()?;

    crate::logging::info("VOICEVOX daemon stopped");
    Ok(())
}
