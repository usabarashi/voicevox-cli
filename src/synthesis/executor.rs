use anyhow::{Context, Result};

use crate::client::DaemonClient;

use super::StreamingSynthesizer;

pub enum PreparedBackend {
    Streaming(StreamingSynthesizer),
    Daemon(DaemonClient),
}

async fn connect_daemon_client_with_retry_context() -> Result<DaemonClient> {
    DaemonClient::connect_with_retry()
        .await
        .context("Failed to connect to VOICEVOX daemon after multiple attempts")
}

/// Prepares a synthesis backend with shared daemon connection policy.
///
/// # Errors
///
/// Returns an error if daemon connection fails or streaming backend construction fails.
pub async fn prepare_backend(streaming: bool) -> Result<PreparedBackend> {
    let client = connect_daemon_client_with_retry_context().await?;
    if streaming {
        Ok(PreparedBackend::Streaming(
            StreamingSynthesizer::new_with_client(client)?,
        ))
    } else {
        Ok(PreparedBackend::Daemon(client))
    }
}
