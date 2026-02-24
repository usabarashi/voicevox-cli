use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::core::VoicevoxCore;
pub(super) struct ModelCatalog {
    style_to_model_map: HashMap<u32, u32>,
    model_default_style_map: HashMap<u32, u32>,
    all_speakers: Vec<crate::voice::Speaker>,
    available_models: Vec<crate::voice::AvailableModel>,
}

impl ModelCatalog {
    // Catalog is intentionally a startup-time snapshot. Runtime model add/remove is not
    // observed until daemon restart under the current fixed-contract architecture.
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

    pub(super) fn new(core: &VoicevoxCore) -> Result<Self> {
        let (mapping, speakers, models) =
            crate::voice::build_style_to_model_map_async_with_progress(core, |_, _, _| {})?;

        Ok(Self {
            model_default_style_map: Self::build_model_default_style_map(&speakers, &mapping),
            style_to_model_map: mapping,
            all_speakers: speakers,
            available_models: models,
        })
    }

    pub(super) fn resolve_synthesis_target(&self, requested_id: u32) -> Result<(u32, u32), String> {
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

    pub(super) fn get_model_path(&self, model_id: u32) -> Option<&Path> {
        self.available_models
            .iter()
            .find(|model| model.model_id == model_id)
            .map(|model| model.file_path.as_path())
    }

    pub(super) fn speakers(&self) -> &[crate::voice::Speaker] {
        &self.all_speakers
    }

    pub(super) fn style_to_model_map(&self) -> &HashMap<u32, u32> {
        &self.style_to_model_map
    }

    pub(super) fn available_models(&self) -> &[crate::voice::AvailableModel] {
        &self.available_models
    }
}
