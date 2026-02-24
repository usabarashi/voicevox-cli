use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::Mutex;

use crate::core::VoicevoxCore;
use crate::ipc::{OwnedRequest, OwnedResponse};

pub struct DaemonState {
    catalog: ModelCatalog,
    synthesis_engine: Mutex<DaemonSynthesisEngine>,
}

struct ModelCatalog {
    style_to_model_map: HashMap<u32, u32>,
    model_default_style_map: HashMap<u32, u32>,
    all_speakers: Vec<crate::voice::Speaker>,
    available_models: Vec<crate::voice::AvailableModel>,
}

struct DaemonSynthesisEngine {
    core: VoicevoxCore,
}

impl ModelCatalog {
    fn build_model_default_style_map(
        speakers: &[crate::voice::Speaker],
        style_to_model_map: &HashMap<u32, u32>,
    ) -> HashMap<u32, u32> {
        speakers
            .iter()
            .flat_map(|speaker| speaker.styles.iter())
            .filter_map(|style| {
                style_to_model_map
                    .get(&style.id)
                    .copied()
                    .map(|model_id| (model_id, style.id))
            })
            .fold(HashMap::new(), |mut acc, (model_id, style_id)| {
                acc.entry(model_id)
                    .and_modify(|current_style_id| {
                        *current_style_id = (*current_style_id).min(style_id);
                    })
                    .or_insert(style_id);
                acc
            })
    }

    fn new(core: &VoicevoxCore) -> Result<Self> {
        let (mapping, speakers, models) =
            crate::voice::build_style_to_model_map_async_with_progress(core, |_, _, _| {})?;

        Ok(Self {
            model_default_style_map: Self::build_model_default_style_map(&speakers, &mapping),
            style_to_model_map: mapping,
            all_speakers: speakers,
            available_models: models,
        })
    }

    fn resolve_synthesis_target(&self, requested_id: u32) -> Result<(u32, u32), String> {
        if let Some(model_id) = self.style_to_model_map.get(&requested_id).copied() {
            return Ok((requested_id, model_id));
        }

        if self
            .available_models
            .iter()
            .any(|model| model.model_id == requested_id)
        {
            let style_id = self
                .model_default_style_map
                .get(&requested_id)
                .copied()
                .ok_or_else(|| format!("Model {requested_id} has no resolvable style IDs"))?;
            return Ok((style_id, requested_id));
        }

        Err(format!(
            "Unknown style/model ID {requested_id}. Use --list-speakers or --list-models to inspect available IDs."
        ))
    }

    fn get_model_path(&self, model_id: u32) -> Option<&Path> {
        self.available_models
            .iter()
            .find(|model| model.model_id == model_id)
            .map(|model| model.file_path.as_path())
    }

    fn speakers_list_response(&self) -> OwnedResponse {
        OwnedResponse::SpeakersListWithModels {
            speakers: self.all_speakers.clone(),
            style_to_model: self.style_to_model_map.clone(),
        }
    }

    fn models_list_response(&self) -> OwnedResponse {
        OwnedResponse::ModelsList {
            models: self.available_models.clone(),
        }
    }
}

impl DaemonSynthesisEngine {
    fn unload_model_if_known(core: &VoicevoxCore, model_id: u32, model_path: Option<&Path>) {
        let Some(model_path) = model_path else {
            eprintln!("Model {model_id} not found in available models");
            return;
        };

        if let Err(error) = core.unload_voice_model_by_path(model_path) {
            eprintln!("Failed to unload model {model_id}: {error}");
        }
    }

    fn new(core: VoicevoxCore) -> Self {
        Self { core }
    }

    fn synthesize(
        &self,
        catalog: &ModelCatalog,
        text: String,
        requested_id: u32,
        rate: f32,
    ) -> OwnedResponse {
        let (style_id, model_id) = match catalog.resolve_synthesis_target(requested_id) {
            Ok(target) => target,
            Err(message) => return OwnedResponse::Error { message },
        };
        let model_path = catalog.get_model_path(model_id);

        if let Err(error) = self.core.load_specific_model(model_id) {
            eprintln!("Failed to load model {model_id}: {error}");
            return OwnedResponse::Error {
                message: format!("Failed to load model {model_id} for synthesis: {error}"),
            };
        }

        let synthesis_result = self.core.synthesize_with_rate(&text, style_id, rate);
        Self::unload_model_if_known(&self.core, model_id, model_path);

        match synthesis_result {
            Ok(wav_data) => OwnedResponse::SynthesizeResult { wav_data },
            Err(error) => OwnedResponse::Error {
                message: format!("Synthesis failed: {error}"),
            },
        }
    }
}

impl DaemonState {
    /// Builds daemon state and precomputes model/style metadata used by requests.
    ///
    /// # Errors
    ///
    /// Returns an error if VOICEVOX core initialization fails, model discovery fails,
    /// or the style-to-model mapping cannot be constructed.
    pub fn new() -> Result<Self> {
        let core = VoicevoxCore::new()?;
        let catalog = ModelCatalog::new(&core)?;
        let synthesis_engine = Mutex::new(DaemonSynthesisEngine::new(core));

        Ok(Self {
            catalog,
            synthesis_engine,
        })
    }

    pub async fn handle_request(&self, request: OwnedRequest) -> OwnedResponse {
        match request {
            OwnedRequest::Ping => OwnedResponse::Pong,
            OwnedRequest::Synthesize {
                text,
                style_id,
                options,
            } => {
                let engine = self.synthesis_engine.lock().await;
                engine.synthesize(&self.catalog, text, style_id, options.rate)
            }
            OwnedRequest::ListSpeakers => self.catalog.speakers_list_response(),
            OwnedRequest::ListModels => self.catalog.models_list_response(),
        }
    }
}
