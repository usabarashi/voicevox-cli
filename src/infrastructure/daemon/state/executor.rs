use std::path::Path;

use crate::infrastructure::core::ModelRuntime;

use super::catalog::{ModelCatalog, TargetResolution};
use super::result::{DaemonServiceError, DaemonServiceErrorKind, DaemonServiceResult};

/// Creates a fresh model runtime per request.
///
/// Per-request creation (rather than a cached core) is the documented memory
/// contract; the factory seam lets the model-based test substitute a recording
/// fake.
pub type ModelRuntimeFactory<R> = Box<dyn Fn() -> anyhow::Result<R> + Send + Sync>;

/// Executes one daemon synthesis request: load a model, synthesize, unload.
#[doc(hidden)]
pub struct DaemonSynthesisExecutor<R: ModelRuntime> {
    create: ModelRuntimeFactory<R>,
}

/// RAII guard that unloads a voice model on drop.
///
/// Guarantees the model is unloaded even on panic or task cancellation (the
/// daemon's per-request load/unload contract), which is modelled by
/// `modeling/quint/ModelLifecycle.qnt`.
struct ModelUnloadGuard<'a, R: ModelRuntime> {
    core: &'a R,
    model_id: u32,
    model_path: Option<&'a Path>,
}

struct AllocatorReliefGuard;

impl Drop for AllocatorReliefGuard {
    fn drop(&mut self) {
        crate::infrastructure::memory::release_unused_allocator_memory();
    }
}

impl<R: ModelRuntime> Drop for ModelUnloadGuard<'_, R> {
    fn drop(&mut self) {
        let Some(model_path) = self.model_path else {
            crate::infrastructure::logging::warn(&format!(
                "Model {} not found in available models",
                self.model_id
            ));
            return;
        };

        if let Err(error) = self.core.unload_model_by_path(model_path) {
            crate::infrastructure::logging::warn(&format!(
                "Failed to unload model {}: {error}",
                self.model_id
            ));
        }
    }
}

impl<R: ModelRuntime> DaemonSynthesisExecutor<R> {
    #[must_use]
    pub fn with_factory(create: ModelRuntimeFactory<R>) -> Self {
        Self { create }
    }

    pub fn synthesize(
        &mut self,
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

        let _allocator_relief = AllocatorReliefGuard;
        let core = (self.create)().map_err(|error| {
            DaemonServiceError::new(
                DaemonServiceErrorKind::ModelLoadFailed,
                format!("Failed to initialize VOICEVOX core for synthesis: {error}"),
            )
        })?;

        if let Err(error) = core.load_model(model_id) {
            crate::infrastructure::logging::error(&format!(
                "Failed to load model {model_id}: {error}"
            ));
            return Err(DaemonServiceError::new(
                DaemonServiceErrorKind::ModelLoadFailed,
                format!("Failed to load model {model_id} for synthesis: {error}"),
            ));
        }

        let synthesis_result = {
            // RAII guard ensures the model is always unloaded, even on panic or
            // task cancellation.
            let _model_guard = ModelUnloadGuard {
                core: &core,
                model_id,
                model_path,
            };

            core.synthesize(&text, style_id, rate)
        };

        match synthesis_result {
            Ok(wav_data) => Ok(DaemonServiceResult::SynthesizeResult { wav_data }),
            Err(error) => Err(DaemonServiceError::new(
                DaemonServiceErrorKind::SynthesisFailed,
                format!("Synthesis failed: {error}"),
            )),
        }
    }
}
