use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

#[cfg(feature = "smallvec")]
use smallvec::SmallVec;

#[cfg(feature = "compact_str")]
use compact_str::CompactString;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Speaker {
    #[cfg(feature = "compact_str")]
    pub name: CompactString,
    #[cfg(not(feature = "compact_str"))]
    pub name: String,

    #[serde(default)]
    #[cfg(feature = "compact_str")]
    pub speaker_uuid: CompactString,
    #[serde(default)]
    #[cfg(not(feature = "compact_str"))]
    pub speaker_uuid: String,

    #[cfg(feature = "smallvec")]
    pub styles: SmallVec<[Style; 8]>,
    #[cfg(not(feature = "smallvec"))]
    pub styles: Vec<Style>,

    #[serde(default)]
    #[cfg(feature = "compact_str")]
    pub version: CompactString,
    #[serde(default)]
    #[cfg(not(feature = "compact_str"))]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    #[cfg(feature = "compact_str")]
    pub name: CompactString,
    #[cfg(not(feature = "compact_str"))]
    pub name: String,

    pub id: u32,

    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    #[cfg(feature = "compact_str")]
    pub style_type: Option<CompactString>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    #[cfg(not(feature = "compact_str"))]
    pub style_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AvailableModel {
    pub model_id: u32,
    pub file_path: PathBuf,
    #[cfg(feature = "smallvec")]
    pub speakers: SmallVec<[Speaker; 4]>,
    #[cfg(not(feature = "smallvec"))]
    pub speakers: Vec<Speaker>,
}

pub fn scan_available_models() -> Result<Vec<AvailableModel>> {
    use crate::paths::find_models_dir_client;

    let models_dir = find_models_dir_client()?;

    #[cfg(feature = "smallvec")]
    let mut available_models = SmallVec::<[AvailableModel; 32]>::new();
    #[cfg(not(feature = "smallvec"))]
    let mut available_models = Vec::new();

    let vvm_files = find_vvm_files(&models_dir)?;

    let models_iter = vvm_files
        .into_iter()
        .filter_map(|vvm_file| {
            extract_model_id_from_path(&vvm_file).map(|model_id| (model_id, vvm_file))
        })
        .map(|(model_id, file_path)| AvailableModel {
            model_id,
            file_path,
            #[cfg(feature = "smallvec")]
            speakers: SmallVec::new(),
            #[cfg(not(feature = "smallvec"))]
            speakers: Vec::new(),
        });

    #[cfg(feature = "rayon")]
    {
        let mut models: Vec<_> = models_iter.collect();
        models.par_sort_unstable_by_key(|m| m.model_id);
        available_models.extend(models);
    }

    #[cfg(not(feature = "rayon"))]
    {
        available_models.extend(models_iter);
        available_models.sort_unstable_by_key(|m| m.model_id);
    }

    #[cfg(feature = "smallvec")]
    let result = available_models.into_vec();
    #[cfg(not(feature = "smallvec"))]
    let result = available_models;

    Ok(result)
}

fn find_vvm_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut vvm_files = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow!("Failed to read directory {}: {}", dir.display(), e))?;

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().map(|ext| ext == "vvm").unwrap_or(false) {
            vvm_files.push(path);
        } else if path.is_dir() {
            // Recursively search subdirectories
            vvm_files.extend(find_vvm_files(&path)?);
        }
    }

    Ok(vvm_files)
}

fn extract_model_id_from_path(path: &Path) -> Option<u32> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .and_then(|stem| stem.parse::<u32>().ok())
        .filter(|&id| id < 10000)
}

pub fn resolve_voice_dynamic(voice_input: &str) -> Result<(u32, String)> {
    if voice_input == "?" {
        const HELP_TEXT: &str = r#"Available VOICEVOX voices:

  Use one of these options to discover voices:
    --list-models        - Show available VVM models
    --list-speakers      - Show all speaker details from loaded models
    --speaker-id N       - Use specific style ID directly
    --model N            - Use model N.vvm

  Examples:
    voicevox-say --speaker-id 3 "text"
    voicevox-say --model 3 "text"
"#;
        println!("{}", HELP_TEXT);
        std::process::exit(0);
    }

    voice_input
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|&id| id > 0 && id < 1000)
        .map(|style_id| (style_id, format!("Style ID {}", style_id)))
        .map(Ok)
        .unwrap_or_else(|| try_resolve_from_available_models(voice_input))
}

fn try_resolve_from_available_models(voice_input: &str) -> Result<(u32, String)> {
    let available_models = scan_available_models().map_err(|e| {
        anyhow!(
            "Failed to scan available models: {}. Use --speaker-id for direct ID specification.",
            e
        )
    })?;

    (!available_models.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!(
            "No voice models available. Please download models first or use --speaker-id for direct ID specification."
        ))?;

    voice_input
        .parse::<u32>()
        .ok()
        .filter(|&model_id| available_models.iter().any(|m| m.model_id == model_id))
        .map(|model_id| (model_id, format!("Model {} (Default Style)", model_id)))
        .map(Ok)
        .unwrap_or_else(|| {
            let model_suggestions = available_models
                .iter()
                .take(3)
                .map(|m| format!("--model {}", m.model_id))
                .collect::<Vec<_>>()
                .join(", ");

            Err(anyhow!(
                "Voice '{}' not found. Available options:\n  \
                Use --speaker-id N for direct style ID\n  \
                Use --model N for model selection (e.g., {})\n  \
                Use --list-models to see all {} available models\n  \
                Use --list-speakers for detailed speaker information",
                voice_input,
                model_suggestions,
                available_models.len()
            ))
        })
}

pub fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    if let Ok(available_models) = scan_available_models() {
        available_models
            .iter()
            .find(|model| {
                model.model_id == voice_id
                    || (voice_id >= model.model_id * 10 && voice_id < (model.model_id + 1) * 10)
            })
            .map(|model| model.model_id)
            .or_else(|| available_models.first().map(|model| model.model_id))
    } else {
        None
    }
}

/// Build style-to-model mapping by scanning all available models dynamically
pub async fn build_style_to_model_map_async(
    core: &crate::core::VoicevoxCore,
) -> Result<(std::collections::HashMap<u32, u32>, Vec<Speaker>)> {
    use crate::core::CoreSynthesis;
    use std::collections::{HashMap, HashSet};

    let mut style_map = HashMap::new();
    let models_dir = crate::paths::find_models_dir()?;

    // First, get the initial state (no models loaded)
    let initial_speakers = core.get_speakers().unwrap_or_default();
    let initial_style_ids: HashSet<u32> = initial_speakers
        .iter()
        .flat_map(|s| s.styles.iter().map(|style| style.id))
        .collect();

    // Process each model file in sorted order
    let mut model_files: Vec<_> = std::fs::read_dir(&models_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("vvm"))
        .collect();
    model_files.sort();

    // Keep track of cumulative style IDs
    let mut cumulative_style_ids = initial_style_ids.clone();

    for path in &model_files {
        // Extract model ID from filename (e.g., "1.vvm" -> 1)
        let model_id = match path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            Some(id) => id,
            None => continue,
        };

        // Temporarily load the model to get its metadata
        match core.load_specific_model(&model_id.to_string()) {
            Ok(_) => {
                // Get speakers after loading this model
                if let Ok(current_speakers) = core.get_speakers() {
                    // Find new style IDs that this model introduced
                    for speaker in current_speakers {
                        for style in speaker.styles {
                            if !cumulative_style_ids.contains(&style.id) {
                                style_map.insert(style.id, model_id);
                                cumulative_style_ids.insert(style.id);
                            }
                        }
                    }
                }

                // Unload the model to save memory
                if let Err(e) = core.unload_voice_model_by_path(path.to_str().unwrap_or("")) {
                    eprintln!(
                        "  ✗ Failed to unload model {} after mapping: {}",
                        model_id, e
                    );
                }
            }
            Err(e) => {
                eprintln!("  ✗ Failed to load model {} for mapping: {}", model_id, e);
            }
        }
    }

    // Load all models to get complete speaker list
    let mut all_speakers = Vec::new();

    // Re-load all models to get the complete speaker list
    for path in &model_files {
        let model_id = match path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            Some(id) => id,
            None => continue,
        };

        if let Err(e) = core.load_specific_model(&model_id.to_string()) {
            eprintln!(
                "  ✗ Failed to reload model {} for speakers: {}",
                model_id, e
            );
        }
    }

    // Get all speakers after loading all models
    if let Ok(speakers) = core.get_speakers() {
        all_speakers = speakers;
    }

    // Unload all models except the preloaded ones
    for path in &model_files {
        if let Err(e) = core.unload_voice_model_by_path(path.to_str().unwrap_or("")) {
            eprintln!("  ✗ Failed to unload model after speaker collection: {}", e);
        }
    }

    Ok((style_map, all_speakers))
}
