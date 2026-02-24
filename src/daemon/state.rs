use crate::ipc::{OwnedRequest, OwnedResponse};

mod catalog;
mod executor;
mod policy;

use anyhow::Result;
use catalog::ModelCatalog;
use executor::DaemonSynthesisExecutor;
use policy::SerializedSynthesisPolicy;

fn daemon_ipc_capabilities() -> Vec<String> {
    [
        "ping",
        "server_info",
        "synthesize",
        "list_speakers",
        "list_models",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub struct DaemonState {
    catalog: ModelCatalog,
    synthesis_policy: SerializedSynthesisPolicy,
}

impl DaemonState {
    /// Builds daemon state and precomputes model/style metadata used by requests.
    ///
    /// # Errors
    ///
    /// Returns an error if VOICEVOX core initialization fails, model discovery fails,
    /// or the style-to-model mapping cannot be constructed.
    pub fn new() -> Result<Self> {
        let core = crate::core::VoicevoxCore::new()?;
        let catalog = ModelCatalog::new(&core)?;
        let synthesis_executor = DaemonSynthesisExecutor::new(core);
        let synthesis_policy = SerializedSynthesisPolicy::new(synthesis_executor);

        Ok(Self {
            catalog,
            synthesis_policy,
        })
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match request {
            OwnedRequest::GetServerInfo => OwnedResponse::ServerInfo {
                protocol_version: crate::ipc::DAEMON_IPC_PROTOCOL_VERSION,
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: daemon_ipc_capabilities(),
            },
            OwnedRequest::Synthesize {
                text,
                style_id,
                options,
            } => {
                self.synthesis_policy
                    .synthesize(&self.catalog, text, style_id, options.rate)
                    .await
            }
            OwnedRequest::ListSpeakers => self.catalog.speakers_list_response(),
            OwnedRequest::ListModels => self.catalog.models_list_response(),
        }
    }
}
