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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Checks if any VOICEVOX models are available in the models directory.
///
/// This function scans the models directory for `.vvm` files and returns
/// `true` if at least one model is found, `false` otherwise.
///
/// # Returns
///
/// * `true` - At least one voice model is available
/// * `false` - No models found or error occurred during scanning
///
/// # Example
///
/// ```no_run
/// use voicevox_cli::voice::has_available_models;
///
/// if has_available_models() {
///     println!("Models are available");
/// } else {
///     println!("Please download models first");
/// }
/// ```
pub fn has_available_models() -> bool {
    use crate::paths::find_models_dir_client;

    find_models_dir_client()
        .ok()
        .map(|dir| has_any_vvm_file(&dir))
        .unwrap_or(false)
}

/// Quickly checks if any .vvm file exists in the given directory (recursively).
/// Returns true as soon as the first .vvm file is found.
fn has_any_vvm_file(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(Result::ok) {
            if let Ok(file_type) = entry.file_type() {
                let path = entry.path();
                if file_type.is_dir() {
                    if has_any_vvm_file(&path) {
                        return true;
                    }
                } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "vvm") {
                    return true;
                }
            }
        }
    }
    false
}

fn find_vvm_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut vvm_files = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow!("Failed to read directory {}: {e}", dir.display()))?;

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && path.extension().map(|ext| ext == "vvm").unwrap_or(false) {
            vvm_files.push(path);
        } else if path.is_dir() {
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
        println!("{HELP_TEXT}");
        std::process::exit(0);
    }

    voice_input
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|&id| id > 0 && id < 1000)
        .map(|style_id| (style_id, format!("Style ID {style_id}")))
        .map(Ok)
        .unwrap_or_else(|| try_resolve_from_available_models(voice_input))
}

fn try_resolve_from_available_models(voice_input: &str) -> Result<(u32, String)> {
    let available_models = scan_available_models().map_err(|e| {
        anyhow!(
            "Failed to scan available models: {e}. Use --speaker-id for direct ID specification."
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
        .map(|model_id| (model_id, format!("Model {model_id} (Default Style)")))
        .map(Ok)
        .unwrap_or_else(|| {
            let model_suggestions = available_models
                .iter()
                .take(3)
                .map(|m| format!("--model {}", m.model_id))
                .collect::<Vec<_>>()
                .join(", ");

            Err(anyhow!(
                "Voice '{voice_input}' not found. Available options:\n  \
                Use --speaker-id N for direct style ID\n  \
                Use --model N for model selection (e.g., {model_suggestions})\n  \
                Use --list-models to see all {} available models\n  \
                Use --list-speakers for detailed speaker information",
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
) -> Result<(
    std::collections::HashMap<u32, u32>,
    Vec<Speaker>,
    Vec<AvailableModel>,
)> {
    build_style_to_model_map_async_with_progress(core, |_, _, _| {}).await
}

pub async fn build_style_to_model_map_async_with_progress<F>(
    core: &crate::core::VoicevoxCore,
    mut progress_callback: F,
) -> Result<(
    std::collections::HashMap<u32, u32>,
    Vec<Speaker>,
    Vec<AvailableModel>,
)>
where
    F: FnMut(usize, usize, &str),
{
    use crate::core::CoreSynthesis;
    use std::collections::{HashMap, HashSet};

    let mut style_map = HashMap::new();
    let models_dir = crate::paths::find_models_dir()?;

    let initial_speakers = core.get_speakers().unwrap_or_default();
    let initial_style_ids: HashSet<u32> = initial_speakers
        .iter()
        .flat_map(|s| s.styles.iter().map(|style| style.id))
        .collect();

    let mut model_files: Vec<_> = std::fs::read_dir(&models_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("vvm"))
        .collect();
    model_files.sort();

    let total_models = model_files.len();
    let mut cumulative_style_ids = initial_style_ids.clone();

    for (index, path) in model_files.iter().enumerate() {
        let model_id = match path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            Some(id) => id,
            None => continue,
        };

        let model_filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.vvm");

        progress_callback(index + 1, total_models, model_filename);

        if core.load_specific_model(&model_id.to_string()).is_err() {
            continue;
        }

        let current_speakers = match core.get_speakers() {
            Ok(speakers) => speakers,
            Err(_) => {
                let path_str = match path.to_str() {
                    Some(s) => s,
                    None => {
                        continue;
                    }
                };
                let _ = core.unload_voice_model_by_path(path_str);
                continue;
            }
        };

        for speaker in current_speakers {
            for style in speaker.styles {
                if cumulative_style_ids.contains(&style.id) {
                    continue;
                }
                style_map.insert(style.id, model_id);
                cumulative_style_ids.insert(style.id);
            }
        }

        let path_str = match path.to_str() {
            Some(s) => s,
            None => {
                continue;
            }
        };
        let _ = core.unload_voice_model_by_path(path_str);
    }

    let mut all_speakers = Vec::new();

    for path in &model_files {
        let model_id = match path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok())
        {
            Some(id) => id,
            None => continue,
        };

        let _ = core.load_specific_model(&model_id.to_string());
    }

    if let Ok(speakers) = core.get_speakers() {
        all_speakers = speakers;
    }

    for path in &model_files {
        let path_str = match path.to_str() {
            Some(s) => s,
            None => {
                continue;
            }
        };
        let _ = core.unload_voice_model_by_path(path_str);
    }

    // Build available models list using existing scan function
    let available_models = scan_available_models().unwrap_or_default();

    Ok((style_map, all_speakers, available_models))
}
