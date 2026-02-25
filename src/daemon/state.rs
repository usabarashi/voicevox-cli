use crate::ipc::{DaemonErrorCode, OwnedRequest, OwnedResponse};

mod catalog;
mod executor;
mod policy;
mod result;

use crate::synthesis::{validate_basic_request, TextSynthesisRequest};
use anyhow::Result;
use catalog::ModelCatalog;
use executor::DaemonSynthesisExecutor;
use policy::SerializedSynthesisPolicy;
use result::{DaemonServiceError, DaemonServiceErrorKind, DaemonServiceResult};

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

    fn to_ipc_error(error: DaemonServiceError) -> OwnedResponse {
        let code = match error.kind {
            DaemonServiceErrorKind::InvalidTargetId => DaemonErrorCode::InvalidTargetId,
            DaemonServiceErrorKind::ModelLoadFailed => DaemonErrorCode::ModelLoadFailed,
            DaemonServiceErrorKind::SynthesisFailed => DaemonErrorCode::SynthesisFailed,
        };
        OwnedResponse::Error {
            code,
            message: error.message,
        }
    }

    fn to_ipc_response(result: DaemonServiceResult) -> OwnedResponse {
        match result {
            DaemonServiceResult::SynthesizeResult { wav_data } => {
                OwnedResponse::SynthesizeResult { wav_data }
            }
            DaemonServiceResult::SpeakersListWithModels {
                speakers,
                style_to_model,
            } => OwnedResponse::SpeakersListWithModels {
                speakers,
                style_to_model,
            },
            DaemonServiceResult::ModelsList { models } => OwnedResponse::ModelsList { models },
        }
    }

    async fn execute_request(
        &self,
        request: OwnedRequest,
    ) -> Result<DaemonServiceResult, DaemonServiceError> {
        match request {
            OwnedRequest::Synthesize {
                text,
                style_id,
                options,
            } => {
                validate_basic_request(&TextSynthesisRequest {
                    text: &text,
                    style_id,
                    rate: options.rate,
                })
                .map_err(|error| {
                    DaemonServiceError::new(
                        DaemonServiceErrorKind::SynthesisFailed,
                        format!("Invalid synthesis request: {error}"),
                    )
                })?;

                self.synthesis_policy
                    .synthesize(&self.catalog, text, style_id, options.rate)
                    .await
            }
            OwnedRequest::ListSpeakers => Ok(DaemonServiceResult::SpeakersListWithModels {
                speakers: self.catalog.speakers().to_vec(),
                style_to_model: self.catalog.style_to_model_map().clone(),
            }),
            OwnedRequest::ListModels => Ok(DaemonServiceResult::ModelsList {
                models: self.catalog.available_models().to_vec(),
            }),
        }
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match self.execute_request(request).await {
            Ok(result) => Self::to_ipc_response(result),
            Err(error) => Self::to_ipc_error(error),
        }
    }
}
