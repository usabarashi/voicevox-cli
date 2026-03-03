pub mod error;
mod launcher;
pub mod policy;
mod transport;

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;
use tokio::net::UnixStream;

use crate::infrastructure::paths::get_socket_path;
use crate::infrastructure::voicevox::{AvailableModel, Speaker};
use crate::interface::ipc::{OwnedRequest, OwnedResponse, OwnedSynthesizeOptions};

pub use crate::infrastructure::daemon::find_daemon_binary;
pub use error::{
    daemon_response_error, daemon_rpc_exit_code, find_daemon_rpc_error,
    format_daemon_rpc_error_for_cli, infer_voice_target_state, DaemonRpcError, VoiceTargetState,
};
pub use policy::{DaemonAutoStartPolicy, DaemonConnectRetryPolicy};

fn unexpected_daemon_response(operation: &str, expected: &str) -> anyhow::Error {
    anyhow!("Daemon returned an unexpected response while {operation} (expected: {expected})")
}

pub struct DaemonRpcClient {
    stream: UnixStream,
}

impl DaemonRpcClient {
    async fn from_stream(stream: UnixStream) -> Result<Self> {
        Ok(Self { stream })
    }

    pub async fn new() -> Result<Self> {
        Self::new_at(&get_socket_path()).await
    }

    pub async fn new_at(socket_path: &Path) -> Result<Self> {
        let stream = transport::connect_socket_with_timeout(
            socket_path,
            transport::DAEMON_CONNECTION_TIMEOUT,
        )
        .await?;
        Self::from_stream(stream).await
    }

    pub async fn connect_with_retry() -> Result<Self> {
        Self::connect_with_retry_at(&get_socket_path()).await
    }

    pub async fn connect_with_retry_at(socket_path: &Path) -> Result<Self> {
        let policy = DaemonConnectRetryPolicy::default();
        let stream = transport::connect_with_retry(
            socket_path,
            transport::DAEMON_CONNECTION_TIMEOUT,
            policy,
        )
        .await?;
        Self::from_stream(stream).await
    }

    pub async fn new_with_auto_start() -> Result<Self> {
        Self::new_with_auto_start_at(&get_socket_path()).await
    }

    pub async fn new_with_auto_start_at(socket_path: &Path) -> Result<Self> {
        let stream = launcher::connect_or_start(socket_path).await?;
        Self::from_stream(stream).await
    }

    async fn send_request_and_receive_response(
        &mut self,
        request: OwnedRequest,
    ) -> Result<OwnedResponse> {
        transport::send_request_and_receive_response(&mut self.stream, &request).await
    }

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
            OwnedResponse::Error { code, message } => {
                Err(daemon_response_error("Synthesis error", code, &message))
            }
            _ => Err(unexpected_daemon_response(
                "handling synthesize request",
                "SynthesizeResult or Error",
            )),
        }
    }

    pub async fn list_speakers(&mut self) -> Result<Vec<Speaker>> {
        match self
            .send_request_and_receive_response(OwnedRequest::ListSpeakers)
            .await?
        {
            OwnedResponse::SpeakersListWithModels { speakers, .. } => Ok(speakers),
            OwnedResponse::Error { code, message } => {
                Err(daemon_response_error("List speakers error", code, &message))
            }
            _ => Err(unexpected_daemon_response(
                "listing speakers",
                "SpeakersListWithModels or Error",
            )),
        }
    }

    pub async fn list_speakers_with_models(&mut self) -> Result<(Vec<Speaker>, HashMap<u32, u32>)> {
        match self
            .send_request_and_receive_response(OwnedRequest::ListSpeakers)
            .await?
        {
            OwnedResponse::SpeakersListWithModels {
                speakers,
                style_to_model,
            } => Ok((speakers, style_to_model)),
            OwnedResponse::Error { code, message } => {
                Err(daemon_response_error("List speakers error", code, &message))
            }
            _ => Err(unexpected_daemon_response(
                "listing speakers with model mapping",
                "SpeakersListWithModels or Error",
            )),
        }
    }

    pub async fn list_models(&mut self) -> Result<Vec<AvailableModel>> {
        match self
            .send_request_and_receive_response(OwnedRequest::ListModels)
            .await?
        {
            OwnedResponse::ModelsList { models } => Ok(models),
            OwnedResponse::Error { code, message } => {
                Err(daemon_response_error("List models error", code, &message))
            }
            _ => Err(unexpected_daemon_response(
                "listing models",
                "ModelsList or Error",
            )),
        }
    }
}
