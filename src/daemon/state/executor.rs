use std::path::Path;

use crate::core::VoicevoxCore;

use super::catalog::ModelCatalog;
use super::result::{DaemonServiceError, DaemonServiceErrorKind, DaemonServiceResult};

pub(super) struct DaemonSynthesisExecutor {
    core: VoicevoxCore,
}

impl DaemonSynthesisExecutor {
    pub(super) fn new(core: VoicevoxCore) -> Self {
        Self { core }
    }

    fn unload_model_if_known(core: &VoicevoxCore, model_id: u32, model_path: Option<&Path>) {
        let Some(model_path) = model_path else {
            crate::logging::warn(&format!("Model {model_id} not found in available models"));
            return;
        };

        if let Err(error) = core.unload_voice_model_by_path(model_path) {
            crate::logging::warn(&format!("Failed to unload model {model_id}: {error}"));
        }
    }

    pub(super) fn synthesize(
        &self,
        catalog: &ModelCatalog,
        text: String,
        requested_id: u32,
        rate: f32,
    ) -> Result<DaemonServiceResult, DaemonServiceError> {
        let (style_id, model_id) = match catalog.resolve_synthesis_target(requested_id) {
            Ok(target) => target,
            Err(message) => {
                return Err(DaemonServiceError::new(
                    DaemonServiceErrorKind::InvalidTargetId,
                    message,
                ));
            }
        };
        let model_path = catalog.get_model_path(model_id);

        if let Err(error) = self.core.load_specific_model(model_id) {
            crate::logging::error(&format!("Failed to load model {model_id}: {error}"));
            return Err(DaemonServiceError::new(
                DaemonServiceErrorKind::ModelLoadFailed,
                format!("Failed to load model {model_id} for synthesis: {error}"),
            ));
        }

        let synthesis_result = self.core.synthesize_with_rate(&text, style_id, rate);
        Self::unload_model_if_known(&self.core, model_id, model_path);

        match synthesis_result {
            Ok(wav_data) => Ok(DaemonServiceResult::SynthesizeResult { wav_data }),
            Err(error) => Err(DaemonServiceError::new(
                DaemonServiceErrorKind::SynthesisFailed,
                format!("Synthesis failed: {error}"),
            )),
        }
    }
}
