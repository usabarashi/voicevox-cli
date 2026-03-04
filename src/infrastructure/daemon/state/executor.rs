use std::path::Path;

use crate::infrastructure::core::VoicevoxCore;

use super::catalog::{ModelCatalog, TargetResolution};
use super::result::{DaemonServiceError, DaemonServiceErrorKind, DaemonServiceResult};

pub(super) struct DaemonSynthesisExecutor {
    core: VoicevoxCore,
}

/// RAII guard that unloads a voice model on drop.
///
/// Guarantees `model_loaded = FALSE` even on panic or task cancellation,
/// matching `DaemonRequestHandling.tla` `ClientDisconnect`:
///   `mutex_holder = c => model_loaded' = FALSE`
struct ModelUnloadGuard<'a> {
    core: &'a VoicevoxCore,
    model_id: u32,
    model_path: Option<&'a Path>,
}

impl Drop for ModelUnloadGuard<'_> {
    fn drop(&mut self) {
        let Some(model_path) = self.model_path else {
            crate::infrastructure::logging::warn(&format!(
                "Model {} not found in available models",
                self.model_id
            ));
            return;
        };

        if let Err(error) = self.core.unload_voice_model_by_path(model_path) {
            crate::infrastructure::logging::warn(&format!(
                "Failed to unload model {}: {error}",
                self.model_id
            ));
        }
    }
}

impl DaemonSynthesisExecutor {
    pub(super) fn new(core: VoicevoxCore) -> Self {
        Self { core }
    }

    pub(super) fn synthesize(
        &self,
        catalog: &ModelCatalog,
        text: String,
        requested_id: u32,
        rate: f32,
    ) -> Result<DaemonServiceResult, DaemonServiceError> {
        let (style_id, model_id) = match catalog.resolve_synthesis_target(requested_id) {
            TargetResolution::Exists { style_id, model_id } => (style_id, model_id),
            TargetResolution::Missing { message } => {
                return Err(DaemonServiceError::new(
                    DaemonServiceErrorKind::InvalidTargetId,
                    message,
                ));
            }
        };
        let model_path = catalog.get_model_path(model_id);

        if let Err(error) = self.core.load_specific_model(model_id) {
            crate::infrastructure::logging::error(&format!(
                "Failed to load model {model_id}: {error}"
            ));
            return Err(DaemonServiceError::new(
                DaemonServiceErrorKind::ModelLoadFailed,
                format!("Failed to load model {model_id} for synthesis: {error}"),
            ));
        }

        // RAII guard ensures the model is always unloaded, even on panic or
        // task cancellation. Matches DaemonRequestHandling.tla ClientDisconnect:
        //   mutex_holder = c => model_loaded' = FALSE
        let _model_guard = ModelUnloadGuard {
            core: &self.core,
            model_id,
            model_path,
        };

        let synthesis_result = self.core.synthesize_with_rate(&text, style_id, rate);

        match synthesis_result {
            Ok(wav_data) => Ok(DaemonServiceResult::SynthesizeResult { wav_data }),
            Err(error) => Err(DaemonServiceError::new(
                DaemonServiceErrorKind::SynthesisFailed,
                format!("Synthesis failed: {error}"),
            )),
        }
    }
}
