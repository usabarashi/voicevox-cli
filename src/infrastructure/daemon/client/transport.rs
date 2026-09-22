use anyhow::{Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::Path;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::time::timeout;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use super::policy::DaemonConnectRetryPolicy;
use crate::infrastructure::ipc::{
    MAX_DAEMON_REQUEST_FRAME_BYTES, MAX_DAEMON_RESPONSE_FRAME_BYTES, OwnedRequest, OwnedResponse,
};

pub(crate) const DAEMON_CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const DAEMON_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

fn encode_request_frame(request: &OwnedRequest) -> Result<Vec<u8>> {
    postcard::to_allocvec(request).map_err(|e| anyhow!("Failed to serialize request: {e}"))
}

fn decode_response_frame(frame: &[u8]) -> Result<OwnedResponse> {
    postcard::from_bytes(frame).map_err(|e| anyhow!("Failed to deserialize response: {e}"))
}

fn current_uid() -> u32 {
    // SAFETY: `getuid` has no preconditions.
    unsafe { libc::getuid() }
}

fn validate_socket_path(socket_path: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(socket_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(anyhow!(
                "Failed to inspect daemon socket {}: {error}",
                socket_path.display()
            ));
        }
    };

    if !metadata.file_type().is_socket() {
        return Err(anyhow!(
            "Refusing to connect to non-socket daemon path: {}",
            socket_path.display()
        ));
    }

    let uid = current_uid();
    if metadata.uid() != uid {
        return Err(anyhow!(
            "Refusing to connect to daemon socket owned by another user: {}",
            socket_path.display()
        ));
    }

    let mode = metadata.permissions().mode() & 0o777;
    if mode & 0o022 != 0 {
        return Err(anyhow!(
            "Daemon socket permissions are too permissive (mode {:o}): {}",
            mode,
            socket_path.display()
        ));
    }

    Ok(())
}

fn verify_peer_credentials(stream: &UnixStream) -> Result<()> {
    let cred = stream
        .peer_cred()
        .map_err(|error| anyhow!("Failed to read daemon peer credentials: {error}"))?;
    let peer_uid = cred.uid();
    let uid = current_uid();
    if peer_uid != uid {
        return Err(anyhow!(
            "Refusing daemon connection from different uid (expected {uid}, got {peer_uid})"
        ));
    }
    Ok(())
}

fn daemon_response_codec() -> LengthDelimitedCodec {
    LengthDelimitedCodec::builder()
        .max_frame_length(MAX_DAEMON_RESPONSE_FRAME_BYTES.max(MAX_DAEMON_REQUEST_FRAME_BYTES))
        .new_codec()
}

pub(crate) async fn connect_socket_with_timeout(
    socket_path: &Path,
    timeout_duration: Duration,
) -> Result<UnixStream> {
    validate_socket_path(socket_path)?;
    let stream = timeout(timeout_duration, UnixStream::connect(socket_path))
        .await
        .map_err(|_| anyhow!("Timeout connecting to daemon"))?
        .map_err(|e| {
            anyhow!(
                "Failed to connect to daemon at {}: {e}",
                socket_path.display()
            )
        })?;
    verify_peer_credentials(&stream)?;
    Ok(stream)
}

/// One connection attempt.
///
/// Implemented by the real socket connector; the model-based test
/// `mbt/tests/mcp_connect.rs` substitutes a counting fake to check the connect
/// budget (`modeling/quint/MCPServer.qnt`).
#[doc(hidden)]
#[allow(async_fn_in_trait)]
pub trait ConnectAttempt {
    type Output;
    type Error;

    async fn connect_once(&mut self) -> Result<Self::Output, Self::Error>;
}

/// The connect retry loop used by [`connect_with_retry`]: up to `attempts`
/// attempts with exponential backoff, then one final attempt without sleep.
///
/// Returns the result and the number of attempts made.
#[doc(hidden)]
pub async fn retry_with_final<A: ConnectAttempt>(
    connector: &mut A,
    attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
) -> (Result<A::Output, A::Error>, u32) {
    let mut retry_delay = initial_delay;
    let mut calls = 0u32;

    for attempt in 0..attempts {
        calls += 1;
        match connector.connect_once().await {
            Ok(value) => return (Ok(value), calls),
            Err(_) => {
                if attempt + 1 < attempts {
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(max_delay);
                }
            }
        }
    }

    // Final connect check without backoff sleep, matching the modeled FinalConnect step.
    calls += 1;
    (connector.connect_once().await, calls)
}

struct SocketConnector<'a> {
    socket_path: &'a Path,
    timeout_duration: Duration,
}

impl ConnectAttempt for SocketConnector<'_> {
    type Output = UnixStream;
    type Error = anyhow::Error;

    async fn connect_once(&mut self) -> Result<UnixStream> {
        connect_socket_with_timeout(self.socket_path, self.timeout_duration).await
    }
}

pub(crate) async fn connect_with_retry(
    socket_path: &Path,
    timeout_duration: Duration,
    policy: DaemonConnectRetryPolicy,
) -> Result<UnixStream> {
    let mut connector = SocketConnector {
        socket_path,
        timeout_duration,
    };
    let (result, _attempts) = retry_with_final(
        &mut connector,
        policy.attempts,
        policy.initial_delay,
        policy.max_delay,
    )
    .await;
    result
}

pub(crate) async fn send_request_and_receive_response(
    stream: &mut UnixStream,
    request: &OwnedRequest,
) -> Result<OwnedResponse> {
    let request_data = encode_request_frame(request)?;
    let mut framed = Framed::new(stream, daemon_response_codec());
    framed.send(request_data.into()).await?;
    let response_data = timeout(DAEMON_RESPONSE_TIMEOUT, framed.next())
        .await
        .map_err(|_| anyhow!("Daemon response timeout"))?
        .ok_or_else(|| anyhow!("No response from daemon"))??;
    decode_response_frame(&response_data)
}
