use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

#[cfg(feature = "smallvec")]
use smallvec::SmallVec;

#[cfg(feature = "compact_str")]
use compact_str::CompactString;

#[cfg(feature = "compact_str")]
type VoiceString = CompactString;
#[cfg(not(feature = "compact_str"))]
type VoiceString = String;

#[cfg(feature = "smallvec")]
type StyleList = SmallVec<[Style; 8]>;
#[cfg(not(feature = "smallvec"))]
type StyleList = Vec<Style>;

#[cfg(feature = "smallvec")]
type SpeakerList = SmallVec<[Speaker; 4]>;
#[cfg(not(feature = "smallvec"))]
type SpeakerList = Vec<Speaker>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Speaker {
    pub name: VoiceString,

    #[serde(default)]
    pub speaker_uuid: VoiceString,

    pub styles: StyleList,

    #[serde(default)]
    pub version: VoiceString,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    pub name: VoiceString,

    pub id: u32,

    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub style_type: Option<VoiceString>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableModel {
    pub model_id: u32,
    pub file_path: PathBuf,
    pub speakers: SpeakerList,
}

pub type StyleModelMapBuildResult = (
    std::collections::HashMap<u32, u32>,
    Vec<Speaker>,
    Vec<AvailableModel>,
);

fn available_models_from_paths(model_files: Vec<PathBuf>) -> Vec<AvailableModel> {
    model_files
        .into_iter()
        .filter_map(|file_path| {
            extract_model_id_from_path(&file_path).map(|model_id| AvailableModel {
                model_id,
                file_path,
                speakers: SpeakerList::new(),
            })
        })
        .collect()
}

fn available_models_from_entries(model_entries: Vec<(u32, PathBuf)>) -> Vec<AvailableModel> {
    model_entries
        .into_iter()
        .map(|(model_id, file_path)| AvailableModel {
            model_id,
            file_path,
            speakers: SpeakerList::new(),
        })
        .collect()
}

fn sort_models_by_id(models: &mut [AvailableModel]) {
    #[cfg(feature = "rayon")]
    {
        models.par_sort_unstable_by_key(|m| m.model_id);
    }

    #[cfg(not(feature = "rayon"))]
    {
        models.sort_unstable_by_key(|m| m.model_id);
    }
}

fn record_new_style_ids<I>(
    style_map: &mut std::collections::HashMap<u32, u32>,
    cumulative_style_ids: &mut std::collections::HashSet<u32>,
    model_id: u32,
    style_ids: I,
) where
    I: IntoIterator<Item = u32>,
{
    style_ids.into_iter().for_each(|style_id| {
        if cumulative_style_ids.insert(style_id) {
            style_map.insert(style_id, model_id);
        }
    });
}

fn unload_model_quietly(core: &crate::core::VoicevoxCore, model_path: &Path) {
    if let Err(error) = core.unload_voice_model_by_path(model_path) {
        eprintln!(
            "Warning: failed to unload model {}: {error}",
            model_path.display()
        );
    }
}

/// Scans the configured models directory for available VOICEVOX model files.
///
/// # Errors
///
/// Returns an error if the models directory cannot be resolved or directory traversal fails.
pub fn scan_available_models() -> Result<Vec<AvailableModel>> {
    use crate::paths::find_models_dir_client;

    let models_dir = find_models_dir_client()?;
    let vvm_files = find_vvm_files(&models_dir)?;
    let mut models = available_models_from_paths(vvm_files);
    sort_models_by_id(&mut models);
    Ok(models)
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
#[must_use]
pub fn has_available_models() -> bool {
    use crate::paths::find_models_dir_client;

    find_models_dir_client()
        .ok()
        .is_some_and(|dir| has_any_vvm_file(&dir))
}

/// Quickly checks if any .vvm file exists in the given directory (recursively).
/// Returns true as soon as the first .vvm file is found.
fn has_any_vvm_file(dir: &Path) -> bool {
    if !dir.exists() {
        return false;
    }

    std::fs::read_dir(dir).ok().is_some_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let Ok(file_type) = entry.file_type() else {
                return false;
            };
            let path = entry.path();
            (file_type.is_file() && path.extension().is_some_and(|ext| ext == "vvm"))
                || (file_type.is_dir() && has_any_vvm_file(&path))
        })
    })
}

fn find_vvm_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    std::fs::read_dir(dir)
        .map_err(|e| anyhow!("Failed to read directory {}: {e}", dir.display()))?
        .try_fold(Vec::new(), |mut files, entry_result| {
            let entry = entry_result
                .map_err(|e| anyhow!("Failed to read entry in {}: {e}", dir.display()))?;
            let file_type = entry
                .file_type()
                .map_err(|e| anyhow!("Failed to inspect entry in {}: {e}", dir.display()))?;
            let path = entry.path();
            if file_type.is_file() && path.extension().is_some_and(|ext| ext == "vvm") {
                files.push(path);
            } else if file_type.is_dir() {
                files.extend(find_vvm_files(&path)?);
            }
            Ok(files)
        })
}

fn extract_model_id_from_path(path: &Path) -> Option<u32> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .and_then(|stem| stem.parse::<u32>().ok())
        .filter(|&id| id < 10000)
}

fn scan_model_file_entries(models_dir: &Path) -> Result<Vec<(u32, PathBuf)>> {
    let mut entries = find_vvm_files(models_dir)?
        .into_iter()
        .filter_map(|path| extract_model_id_from_path(&path).map(|model_id| (model_id, path)))
        .collect::<Vec<_>>();
    entries.sort_unstable_by_key(|(model_id, _)| *model_id);
    Ok(entries)
}

/// Resolves a CLI voice input string into a style/model ID and description.
///
/// # Errors
///
/// Returns an error if model discovery fails or the provided voice/model specifier cannot
/// be resolved to an available voice.
pub fn resolve_voice_dynamic(voice_input: &str) -> Result<(u32, String)> {
    let voice_input = voice_input.trim();

    if voice_input == "?" {
        return Err(anyhow!(
            "Voice help is a CLI concern. Call `print_voice_help()` before resolving."
        ));
    }

    voice_input
        .parse::<u32>()
        .ok()
        .filter(|&id| id > 0 && id < 1000)
        .map(|style_id| (style_id, format!("Style ID {style_id}")))
        .map_or_else(|| try_resolve_from_available_models(voice_input), Ok)
}

pub fn print_voice_help() {
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
}

fn try_resolve_from_available_models(voice_input: &str) -> Result<(u32, String)> {
    let voice_input = voice_input.trim();
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
        .map_or_else(
            || {
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
            },
            Ok,
        )
}

#[must_use]
pub fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    scan_available_models().ok().and_then(|available_models| {
        available_models
            .iter()
            .find(|model| {
                model.model_id == voice_id
                    || (voice_id >= model.model_id * 10 && voice_id < (model.model_id + 1) * 10)
            })
            .map(|model| model.model_id)
            .or_else(|| available_models.first().map(|model| model.model_id))
    })
}

/// Build style-to-model mapping by scanning all available models dynamically
///
/// # Errors
///
/// Returns an error if model directory scanning fails or model metadata extraction fails.
pub fn build_style_to_model_map_async(
    core: &crate::core::VoicevoxCore,
) -> Result<StyleModelMapBuildResult> {
    build_style_to_model_map_async_with_progress(core, |_, _, _| {})
}

/// Builds a style-to-model map while reporting progress for each scanned model file.
///
/// # Errors
///
/// Returns an error if model directory scanning fails or core speaker metadata cannot be
/// queried for the initial state.
pub fn build_style_to_model_map_async_with_progress<F>(
    core: &crate::core::VoicevoxCore,
    mut progress_callback: F,
) -> Result<StyleModelMapBuildResult>
where
    F: FnMut(usize, usize, &str),
{
    use crate::core::CoreSynthesis;
    use std::collections::{HashMap, HashSet};

    let mut style_map = HashMap::new();
    let models_dir = crate::paths::find_models_dir()?;

    let initial_speakers = core.get_speakers()?;
    let initial_style_ids: HashSet<u32> = initial_speakers
        .iter()
        .flat_map(|s| s.styles.iter().map(|style| style.id))
        .collect();

    let model_entries = scan_model_file_entries(&models_dir)?;
    let total_models = model_entries.len();
    let mut cumulative_style_ids = initial_style_ids;

    for (index, (model_id, path)) in model_entries.iter().enumerate() {
        let model_filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.vvm");

        progress_callback(index + 1, total_models, model_filename);

        if core.load_specific_model(&model_id.to_string()).is_err() {
            continue;
        }

        let Ok(current_speakers) = core.get_speakers() else {
            unload_model_quietly(core, path);
            continue;
        };

        record_new_style_ids(
            &mut style_map,
            &mut cumulative_style_ids,
            *model_id,
            current_speakers
                .into_iter()
                .flat_map(|speaker| speaker.styles.into_iter().map(|style| style.id)),
        );

        unload_model_quietly(core, path);
    }

    let loaded_model_paths = model_entries
        .iter()
        .filter_map(|(model_id, path)| {
            core.load_specific_model(&model_id.to_string())
                .ok()
                .map(|()| path)
        })
        .collect::<Vec<_>>();

    let all_speakers = core.get_speakers()?;

    for path in loaded_model_paths {
        unload_model_quietly(core, path);
    }

    let mut available_models = available_models_from_entries(model_entries);
    sort_models_by_id(&mut available_models);

    Ok((style_map, all_speakers, available_models))
}

#[cfg(test)]
mod tests {
    use super::resolve_voice_dynamic;

    #[test]
    fn resolve_voice_dynamic_trims_direct_style_id() {
        let (style_id, description) =
            resolve_voice_dynamic("  3  ").expect("trimmed numeric style id should resolve");
        assert_eq!(style_id, 3);
        assert_eq!(description, "Style ID 3");
    }
}
