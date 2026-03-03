use anyhow::{anyhow, Result};
use futures_util::{SinkExt, StreamExt};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::timeout;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::daemon::state::DaemonState;
use crate::ipc::{DaemonRequest, OwnedResponse, MAX_DAEMON_REQUEST_FRAME_BYTES};

const SOCKET_DIR_MODE: u32 = 0o700;
const SOCKET_FILE_MODE: u32 = 0o600;
const MAX_CONCURRENT_CLIENTS: usize = 32;
const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

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
    match std::fs::symlink_metadata(socket_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket() {
                return Err(anyhow!(
                    "Refusing to remove non-socket path: {}",
                    socket_path.display()
                ));
            }
            std::fs::remove_file(socket_path)?;
            Ok(())
        }
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
    handle_client_with_limit(
        stream,
        state,
        Arc::new(Semaphore::new(MAX_CONCURRENT_CLIENTS)),
    )
    .await
}

async fn acquire_request_permit(permits: Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
    permits.acquire_owned().await.ok()
}

async fn handle_client_with_limit(
    stream: UnixStream,
    state: Arc<DaemonState>,
    permits: Arc<Semaphore>,
) -> Result<()> {
    let codec = LengthDelimitedCodec::builder()
        .max_frame_length(MAX_DAEMON_REQUEST_FRAME_BYTES)
        .new_codec();
    let mut framed = Framed::new(stream, codec);

    while let Some(frame) = timeout(CLIENT_IDLE_TIMEOUT, framed.next())
        .await
        .map_err(|_| anyhow!("Client idle timeout"))?
    {
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

        // `DaemonRequestHandling.tla` models permit admission per request, not per
        // connection. Acquire/release around request handling to keep that contract.
        let Some(_permit) = acquire_request_permit(Arc::clone(&permits)).await else {
            log_client_error("Permit semaphore closed", &"request limiter unavailable");
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
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT_CLIENTS));
    loop {
        let (stream, _) = listener.accept().await?;
        let state_clone = Arc::clone(&state);
        let permits_clone = Arc::clone(&permits);
        tokio::spawn(async move {
            if let Err(error) = handle_client_with_limit(stream, state_clone, permits_clone).await {
                log_client_error("Client handler error", &error);
            }
        });
    }
}

fn ensure_socket_parent_dir(socket_path: &Path) -> Result<()> {
    if let Some(parent_dir) = socket_path.parent() {
        if !parent_dir.exists() {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            builder.mode(SOCKET_DIR_MODE);
            builder.create(parent_dir)?;
        }
        validate_socket_parent_dir(parent_dir)?;
    }
    Ok(())
}

fn validate_socket_parent_dir(parent_dir: &Path) -> Result<()> {
    let metadata = std::fs::metadata(parent_dir)?;
    if !metadata.is_dir() {
        return Err(anyhow!(
            "Socket parent path is not a directory: {}",
            parent_dir.display()
        ));
    }

    let current_uid = current_uid();
    if metadata.uid() != current_uid {
        return Err(anyhow!(
            "Socket parent directory must be owned by current user: {}",
            parent_dir.display()
        ));
    }

    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(anyhow!(
            "Socket parent directory is too permissive (mode {:o}): {}",
            mode,
            parent_dir.display()
        ));
    }

    Ok(())
}

fn current_uid() -> u32 {
    // SAFETY: `getuid` has no preconditions.
    unsafe { libc::getuid() }
}

fn set_socket_permissions(socket_path: &Path) -> Result<()> {
    std::fs::set_permissions(
        socket_path,
        std::fs::Permissions::from_mode(SOCKET_FILE_MODE),
    )?;
    Ok(())
}

/// Runs the daemon accept loop and serves requests over a Unix domain socket.
///
/// # Errors
///
/// Returns an error if socket bind fails, daemon state initialization fails,
/// socket accept fails, or final socket cleanup fails during shutdown.
///
/// Daemon state (VoicevoxCore, model catalog) is initialized before binding
/// the socket, ensuring the daemon is fully ready before clients can connect.
/// This matches the TLA+ `ConnectedImpliesReady` invariant.
///
/// Stale socket removal is handled by `check_and_prevent_duplicate` before
/// this function is called. The `bind` call is the atomic safety gate:
/// if the socket already exists (another daemon bound it), bind fails
/// with `EADDRINUSE`, matching the TLA+ model's atomic `BindSocket`.
pub async fn run_daemon(socket_path: PathBuf, foreground: bool) -> Result<()> {
    ensure_socket_parent_dir(&socket_path)?;

    let state = Arc::new(DaemonState::new()?);

    let socket_guard = SocketFileGuard::new(socket_path.clone());
    let listener = UnixListener::bind(&socket_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AddrInUse {
            anyhow!(
                "Socket already in use: {}. Another daemon may be running.",
                socket_path.display()
            )
        } else {
            e.into()
        }
    })?;
    set_socket_permissions(&socket_path)?;
    crate::logging::info("VOICEVOX daemon started successfully");
    crate::logging::info(&format!("Listening on: {}", socket_path.display()));

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
