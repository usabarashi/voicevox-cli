mod launcher;
mod rpc;
mod transport;

use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::net::UnixStream;

use crate::ipc::{OwnedRequest, OwnedResponse, OwnedSynthesizeOptions};
use crate::paths::get_socket_path;
use crate::voice::{AvailableModel, Speaker};

pub use crate::daemon::find_daemon_binary;
pub use rpc::{daemon_mode, list_speakers_daemon};

fn daemon_response_error(context: &str, message: &str) -> anyhow::Error {
    anyhow!("{context}: {message}")
}

fn unexpected_daemon_response(context: &str) -> anyhow::Error {
    anyhow!("Unexpected response {context}")
}

pub struct DaemonClient {
    stream: UnixStream,
}

impl DaemonClient {
    /// Connects to the daemon using the default socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon socket cannot be reached.
    pub async fn new() -> Result<Self> {
        Self::new_at(&get_socket_path()).await
    }

    /// Connects to the daemon using an explicit socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon socket cannot be reached.
    pub async fn new_at(socket_path: &Path) -> Result<Self> {
        let stream = transport::connect_socket_with_timeout(
            socket_path,
            transport::DAEMON_CONNECTION_TIMEOUT,
        )
        .await?;
        Ok(Self { stream })
    }

    /// Connects to the daemon with retry/backoff behavior.
    ///
    /// # Errors
    ///
    /// Returns an error if all retry attempts fail.
    pub async fn connect_with_retry() -> Result<Self> {
        Self::connect_with_retry_at(&get_socket_path()).await
    }

    /// Connects to the daemon with retry/backoff behavior using an explicit socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if all retry attempts fail.
    pub async fn connect_with_retry_at(socket_path: &Path) -> Result<Self> {
        use crate::daemon::startup;

        let mut last_error = None;
        let mut retry_delay = startup::initial_retry_delay();

        for attempt in 0..startup::MAX_CONNECT_ATTEMPTS {
            match Self::new_at(socket_path).await {
                Ok(client) => return Ok(client),
                Err(error) => {
                    last_error = Some(error);
                    if attempt < startup::MAX_CONNECT_ATTEMPTS - 1 {
                        tokio::time::sleep(retry_delay).await;
                        retry_delay = (retry_delay * 2).min(startup::max_retry_delay());
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow!(
                "Failed to connect to daemon after {} attempts",
                startup::MAX_CONNECT_ATTEMPTS
            )
        }))
    }

    /// Creates a new `DaemonClient` with automatic daemon startup if not running.
    ///
    /// # Errors
    ///
    /// Returns an error if no models are available, daemon startup fails, or connection fails.
    pub async fn new_with_auto_start() -> Result<Self> {
        Self::new_with_auto_start_at(&get_socket_path()).await
    }

    /// Creates a new `DaemonClient` with automatic daemon startup using an explicit socket path.
    ///
    /// # Errors
    ///
    /// Returns an error if no models are available, daemon startup fails, or connection fails.
    pub async fn new_with_auto_start_at(socket_path: &Path) -> Result<Self> {
        let stream = launcher::connect_or_start(socket_path).await?;
        Ok(Self { stream })
    }

    async fn send_request_and_receive_response(
        &mut self,
        request: OwnedRequest,
    ) -> Result<OwnedResponse> {
        transport::send_request_and_receive_response(&mut self.stream, &request).await
    }

    /// Sends a synthesis request and returns the generated WAV bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if request transmission fails, the response is invalid, or the
    /// daemon reports a synthesis error.
    pub async fn synthesize(
        &mut self,
        text: &str,
        style_id: u32,
        options: OwnedSynthesizeOptions,
    ) -> Result<Vec<u8>> {
        let request = OwnedRequest::Synthesize {
            text: text.to_string(),
            style_id,
            options,
        };

        match self.send_request_and_receive_response(request).await? {
            OwnedResponse::SynthesizeResult { wav_data } => Ok(wav_data),
            OwnedResponse::Error { message } => {
                Err(daemon_response_error("Synthesis error", &message))
            }
            _ => Err(unexpected_daemon_response("type")),
        }
    }

    /// Fetches speakers from the daemon.
    ///
    /// # Errors
    ///
    /// Returns an error if request/response I/O fails, decoding fails, or the daemon
    /// returns an error response.
    pub async fn list_speakers(&mut self) -> Result<Vec<Speaker>> {
        match self
            .send_request_and_receive_response(OwnedRequest::ListSpeakers)
            .await?
        {
            OwnedResponse::SpeakersList { speakers }
            | OwnedResponse::SpeakersListWithModels { speakers, .. } => Ok(speakers),
            OwnedResponse::Error { message } => {
                Err(daemon_response_error("List speakers error", &message))
            }
            _ => Err(unexpected_daemon_response("type")),
        }
    }

    /// Fetches available models from the daemon.
    ///
    /// # Errors
    ///
    /// Returns an error if request/response I/O fails, decoding fails, or the daemon
    /// returns an error response.
    pub async fn list_models(&mut self) -> Result<Vec<AvailableModel>> {
        match self
            .send_request_and_receive_response(OwnedRequest::ListModels)
            .await?
        {
            OwnedResponse::ModelsList { models } => Ok(models),
            OwnedResponse::Error { message } => {
                Err(daemon_response_error("List models error", &message))
            }
            _ => Err(unexpected_daemon_response("type")),
        }
    }
}
