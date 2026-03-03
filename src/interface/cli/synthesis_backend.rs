use anyhow::{Context, Result};

use crate::config::Config;
use crate::interface::cli::DaemonRpcClient;

use super::streaming_synthesizer::StreamingSynthesizer;

pub enum PreparedBackend {
    Streaming(StreamingSynthesizer),
    Daemon(DaemonRpcClient),
}

async fn connect_daemon_rpc_with_retry_context() -> Result<DaemonRpcClient> {
    DaemonRpcClient::connect_with_retry()
        .await
        .context("Failed to connect to VOICEVOX daemon after multiple attempts")
}

/// Prepares a synthesis backend with shared daemon connection policy.
///
/// # Errors
///
/// Returns an error if daemon connection fails or streaming backend construction fails.
pub async fn prepare_backend(streaming: bool) -> Result<PreparedBackend> {
    prepare_backend_with_config(streaming, &Config::default()).await
}

/// Prepares a synthesis backend with injected configuration for streaming behavior.
///
/// # Errors
///
/// Returns an error if daemon connection fails or streaming backend construction fails.
pub async fn prepare_backend_with_config(
    streaming: bool,
    config: &Config,
) -> Result<PreparedBackend> {
    let client = connect_daemon_rpc_with_retry_context().await?;
    if streaming {
        Ok(PreparedBackend::Streaming(
            StreamingSynthesizer::new_with_client_and_config(client, config)?,
        ))
    } else {
        Ok(PreparedBackend::Daemon(client))
    }
}
