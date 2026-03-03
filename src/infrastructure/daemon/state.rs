use crate::infrastructure::ipc::{
    DaemonErrorCode, IpcModel, IpcSpeaker, IpcStyle, OwnedRequest, OwnedResponse,
};

mod catalog;
mod executor;
mod policy;
mod result;

use crate::domain::synthesis::{validate_basic_request, TextSynthesisRequest};
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
    fn to_ipc_style(style: &crate::infrastructure::voicevox::Style) -> IpcStyle {
        IpcStyle {
            name: style.name.to_string(),
            id: style.id,
            style_type: style.style_type.as_ref().map(ToString::to_string),
        }
    }

    fn to_ipc_speaker(speaker: &crate::infrastructure::voicevox::Speaker) -> IpcSpeaker {
        IpcSpeaker {
            name: speaker.name.to_string(),
            speaker_uuid: speaker.speaker_uuid.to_string(),
            styles: speaker.styles.iter().map(Self::to_ipc_style).collect(),
            version: speaker.version.to_string(),
        }
    }

    fn to_ipc_model(model: &crate::infrastructure::voicevox::AvailableModel) -> IpcModel {
        IpcModel {
            model_id: model.model_id,
            file_path: model.file_path.clone(),
            speakers: model.speakers.iter().map(Self::to_ipc_speaker).collect(),
        }
    }

    /// Builds daemon state and precomputes model/style metadata used by requests.
    ///
    /// # Errors
    ///
    /// Returns an error if VOICEVOX core initialization fails, model discovery fails,
    /// or the style-to-model mapping cannot be constructed.
    pub fn new() -> Result<Self> {
        let core = crate::infrastructure::core::VoicevoxCore::new()?;
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
                speakers: speakers.iter().map(Self::to_ipc_speaker).collect(),
                style_to_model,
            },
            DaemonServiceResult::ModelsList { models } => OwnedResponse::ModelsList {
                models: models.iter().map(Self::to_ipc_model).collect(),
            },
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
