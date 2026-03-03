use anyhow::{Context, Result};

use crate::config::Config;
use crate::infrastructure::daemon::client::DaemonClient;

use super::daemon::DaemonSynthesizer;
use super::streaming::StreamingSynthesizer;

pub enum SynthesisMode {
    Streaming(StreamingSynthesizer),
    Daemon(DaemonSynthesizer),
}

async fn connect_daemon_client_with_retry_context() -> Result<DaemonClient> {
    DaemonClient::connect_with_retry()
        .await
        .context("Failed to connect to VOICEVOX daemon after multiple attempts")
}

/// Selects synthesis mode with shared daemon connection policy.
///
/// # Errors
///
/// Returns an error if daemon connection fails or streaming synthesizer construction fails.
pub async fn select_synthesis_mode(streaming: bool) -> Result<SynthesisMode> {
    select_synthesis_mode_with_config(streaming, &Config::default()).await
}

/// Selects synthesis mode with injected configuration for streaming behavior.
///
/// # Errors
///
/// Returns an error if daemon connection fails or streaming synthesizer construction fails.
pub async fn select_synthesis_mode_with_config(
    streaming: bool,
    config: &Config,
) -> Result<SynthesisMode> {
    let client = connect_daemon_client_with_retry_context().await?;
    if streaming {
        Ok(SynthesisMode::Streaming(
            StreamingSynthesizer::new_with_client_and_config(client, config)?,
        ))
    } else {
        Ok(SynthesisMode::Daemon(DaemonSynthesizer::new_with_client(
            client,
        )))
    }
}
