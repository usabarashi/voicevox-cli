use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::path::Path;
use std::time::Duration;
use tokio::net::UnixStream;
use tokio::time::timeout;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::ipc::{OwnedRequest, OwnedResponse};

pub(crate) const DAEMON_CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const DAEMON_RESPONSE_TIMEOUT: Duration = Duration::from_secs(30);

fn encode_request_frame(request: &OwnedRequest) -> Result<Vec<u8>> {
    bincode::serde::encode_to_vec(request, bincode::config::standard())
        .map_err(|e| anyhow!("Failed to serialize request: {e}"))
}

fn decode_response_frame(frame: &[u8]) -> Result<OwnedResponse> {
    bincode::serde::decode_from_slice(frame, bincode::config::standard())
        .map(|(response, _)| response)
        .map_err(|e| anyhow!("Failed to deserialize response: {e}"))
}

pub(crate) async fn connect_socket_with_timeout(
    socket_path: &Path,
    timeout_duration: Duration,
) -> Result<UnixStream> {
    timeout(timeout_duration, UnixStream::connect(socket_path))
        .await
        .map_err(|_| anyhow!("Timeout connecting to daemon"))?
        .map_err(|e| {
            anyhow!(
                "Failed to connect to daemon at {}: {e}",
                socket_path.display()
            )
        })
}

pub(crate) async fn connect_daemon_with_timeout(
    socket_path: &Path,
    timeout_duration: Duration,
) -> Result<UnixStream> {
    connect_socket_with_timeout(socket_path, timeout_duration)
        .await
        .with_context(|| format!("Daemon connection failed at {}", socket_path.display()))
}

pub(crate) async fn request_daemon_once(
    socket_path: &Path,
    request: &OwnedRequest,
    connect_timeout_duration: Duration,
    response_timeout_duration: Duration,
) -> Result<OwnedResponse> {
    let stream = connect_daemon_with_timeout(socket_path, connect_timeout_duration).await?;
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    let request_data = encode_request_frame(request)?;
    framed
        .send(request_data.into())
        .await
        .map_err(|e| anyhow!("Failed to send request: {e}"))?;

    let response_frame = timeout(response_timeout_duration, framed.next())
        .await
        .map_err(|_| anyhow!("Daemon response timeout"))?
        .ok_or_else(|| anyhow!("Connection closed by daemon"))?
        .map_err(|e| anyhow!("Failed to receive response: {e}"))?;

    decode_response_frame(&response_frame)
}

pub(crate) async fn send_request_and_receive_response(
    stream: &mut UnixStream,
    request: &OwnedRequest,
) -> Result<OwnedResponse> {
    let request_data = encode_request_frame(request)?;
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());
    framed.send(request_data.into()).await?;
    let response_data = timeout(DAEMON_RESPONSE_TIMEOUT, framed.next())
        .await
        .map_err(|_| anyhow!("Daemon response timeout"))?
        .ok_or_else(|| anyhow!("No response from daemon"))??;
    decode_response_frame(&response_data)
}
