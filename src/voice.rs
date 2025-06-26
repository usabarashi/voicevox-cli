//! Dynamic voice detection and resolution system

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

#[cfg(feature = "smallvec")]
use smallvec::SmallVec;

#[cfg(feature = "compact_str")]
use compact_str::CompactString;

/// Voice speaker metadata with multiple emotional styles
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Speaker {
    /// Display name of the speaker character
    #[cfg(feature = "compact_str")]
    pub name: CompactString,
    #[cfg(not(feature = "compact_str"))]
    pub name: String,

    /// Unique identifier for the speaker (UUID format)
    #[serde(default)]
    #[cfg(feature = "compact_str")]
    pub speaker_uuid: CompactString,
    #[serde(default)]
    #[cfg(not(feature = "compact_str"))]
    pub speaker_uuid: String,

    /// Available emotional/speaking styles for this speaker
    #[cfg(feature = "smallvec")]
    pub styles: SmallVec<[Style; 8]>,
    #[cfg(not(feature = "smallvec"))]
    pub styles: Vec<Style>,

    /// Voice model version
    #[serde(default)]
    #[cfg(feature = "compact_str")]
    pub version: CompactString,
    #[serde(default)]
    #[cfg(not(feature = "compact_str"))]
    pub version: String,
}

/// Individual voice style within a speaker
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    /// Display name of the style (e.g., "ノーマル", "あまあま", "ツンツン")
    #[cfg(feature = "compact_str")]
    pub name: CompactString,
    #[cfg(not(feature = "compact_str"))]
    pub name: String,

    /// Unique style ID used for speech synthesis
    pub id: u32,

    /// Optional style type classification
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    #[cfg(feature = "compact_str")]
    pub style_type: Option<CompactString>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    #[cfg(not(feature = "compact_str"))]
    pub style_type: Option<String>,
}

/// Available voice model with metadata
#[derive(Debug, Clone)]
pub struct AvailableModel {
    /// Numeric model ID extracted from filename (e.g., 3 from "3.vvm")
    pub model_id: u32,
    /// Full path to the VVM model file
    pub file_path: PathBuf,
    /// List of voice speakers contained in this model
    #[cfg(feature = "smallvec")]
    pub speakers: SmallVec<[Speaker; 4]>,
    #[cfg(not(feature = "smallvec"))]
    pub speakers: Vec<Speaker>,
}

/// Scans for available voice models in the models directory
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

    #[cfg(feature = "smallvec")]
    let mut vvm_files = SmallVec::<[PathBuf; 16]>::new();
    #[cfg(not(feature = "smallvec"))]
    let mut vvm_files = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow!("Failed to read directory {}: {}", dir.display(), e))?;

    let paths: Result<Vec<_>> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .map(|path| process_path_entry(&path))
        .collect();

    let all_paths = paths?;

    for path_result in all_paths {
        match path_result {
            PathProcessResult::VvmFile(path) => vvm_files.push(path),
            PathProcessResult::Directory(paths) => vvm_files.extend(paths),
            PathProcessResult::Skip => continue,
        }
    }

    #[cfg(feature = "smallvec")]
    let result = vvm_files.into_vec();
    #[cfg(not(feature = "smallvec"))]
    let result = vvm_files;

    Ok(result)
}

#[derive(Debug)]
enum PathProcessResult {
    VvmFile(PathBuf),
    Directory(Vec<PathBuf>),
    Skip,
}

fn process_path_entry(path: &PathBuf) -> Result<PathProcessResult> {
    if path.is_file() {
        let is_vvm = path.extension().map(|ext| ext == "vvm").unwrap_or(false);

        if is_vvm {
            Ok(PathProcessResult::VvmFile(path.clone()))
        } else {
            Ok(PathProcessResult::Skip)
        }
    } else if path.is_dir() {
        find_vvm_files(path).map(PathProcessResult::Directory)
    } else {
        Ok(PathProcessResult::Skip)
    }
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
        .unwrap_or_else(|| resolve_voice_with_core_integration(voice_input))
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

pub fn get_styles_for_model_from_core(model_id: u32) -> Result<Vec<Style>> {
    use crate::core::VoicevoxCore;

    match VoicevoxCore::new() {
        Ok(core) => match core.get_speakers() {
            Ok(speakers) => {
                let mut styles = Vec::new();
                for speaker in speakers {
                    for style in speaker.styles {
                        if (style.id >= model_id * 10 && style.id < (model_id + 1) * 10)
                            || (model_id <= 30 && style.id == model_id)
                        {
                            styles.push(style);
                        }
                    }
                }
                Ok(styles)
            }
            Err(e) => Err(anyhow!("Failed to get speakers from VOICEVOX Core: {}", e)),
        },
        Err(_) => Ok(Vec::new()),
    }
}

pub fn resolve_voice_with_core_integration(voice_input: &str) -> Result<(u32, String)> {
    match try_resolve_from_available_models(voice_input) {
        Ok(result) => Ok(result),
        Err(_) => {
            use crate::core::VoicevoxCore;

            if let Ok(core) = VoicevoxCore::new() {
                if let Ok(speakers) = core.get_speakers() {
                    for speaker in speakers {
                        let normalized_name = speaker
                            .name
                            .to_lowercase()
                            .replace(" ", "")
                            .replace("（", "")
                            .replace("）", "")
                            .replace("(", "")
                            .replace(")", "");

                        let normalized_input =
                            voice_input.to_lowercase().replace(" ", "").replace("-", "");

                        if normalized_name.contains(&normalized_input) {
                            if let Some(first_style) = speaker.styles.first() {
                                return Ok((
                                    first_style.id,
                                    format!("{} ({})", speaker.name, first_style.name),
                                ));
                            }
                        }
                    }
                }
            }

            Err(anyhow!(
                "Voice '{}' not found. Use --list-speakers to see all available voices.",
                voice_input
            ))
        }
    }
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
