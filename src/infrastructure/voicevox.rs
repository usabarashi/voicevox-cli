use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use voicevox_core::blocking::{OpenJtalk, Synthesizer, VoiceModelFile};

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

pub type StyleModelMapBuildResult = (HashMap<u32, u32>, Vec<Speaker>, Vec<AvailableModel>);

/// Opens a voice model file from an explicit path.
///
/// # Errors
///
/// Returns an error if the model cannot be opened.
pub fn open_voice_model_file(model_path: &Path) -> Result<VoiceModelFile> {
    VoiceModelFile::open(model_path)
        .map_err(|e| anyhow!("Failed to open model file {}: {e}", model_path.display()))
}

/// Resolves `<model_id>.vvm` in the daemon model directory and opens it.
///
/// # Errors
///
/// Returns an error if the model file does not exist or cannot be opened.
pub fn open_voice_model_file_by_id(model_id: u32) -> Result<VoiceModelFile> {
    let models_dir = crate::infrastructure::paths::find_models_dir()?;
    let model_path = models_dir.join(format!("{model_id}.vvm"));

    if !model_path.exists() {
        return Err(anyhow!(
            "Model not found: {model_id}.vvm at {}",
            models_dir.display()
        ));
    }

    VoiceModelFile::open(&model_path).map_err(|e| anyhow!("Failed to open model {model_id}: {e}"))
}

pub(crate) fn collect_speakers_from_synthesizer(
    synthesizer: &Synthesizer<OpenJtalk>,
) -> Vec<Speaker> {
    synthesizer
        .metas()
        .iter()
        .map(|meta| Speaker {
            name: meta.name.clone().into(),
            speaker_uuid: meta.speaker_uuid.clone().into(),
            styles: meta
                .styles
                .iter()
                .map(|style| Style {
                    name: style.name.clone().into(),
                    id: style.id.0,
                    style_type: Some(format!("{:?}", style.r#type).into()),
                })
                .collect(),
            version: meta.version.to_string().into(),
        })
        .collect()
}

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

fn populate_model_speakers(
    models: &mut [AvailableModel],
    speakers: &[Speaker],
    style_to_model_map: &std::collections::HashMap<u32, u32>,
) {
    for model in models.iter_mut() {
        model.speakers = speakers
            .iter()
            .filter_map(|speaker| {
                let styles = speaker
                    .styles
                    .iter()
                    .filter(|style| {
                        style_to_model_map.get(&style.id).copied() == Some(model.model_id)
                    })
                    .cloned()
                    .collect::<StyleList>();

                (!styles.is_empty()).then(|| Speaker {
                    name: speaker.name.clone(),
                    speaker_uuid: speaker.speaker_uuid.clone(),
                    styles,
                    version: speaker.version.clone(),
                })
            })
            .collect::<SpeakerList>();
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

fn unload_model_quietly(core: &crate::infrastructure::core::VoicevoxCore, model_path: &Path) {
    if let Err(error) = core.unload_voice_model_by_path(model_path) {
        crate::infrastructure::logging::warn(&format!(
            "Failed to unload model {}: {error}",
            model_path.display()
        ));
    }
}

fn is_vvm_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("vvm"))
}

/// Scans the configured models directory for available VOICEVOX model files.
///
/// # Errors
///
/// Returns an error if the models directory cannot be resolved or directory traversal fails.
pub fn scan_available_models() -> Result<Vec<AvailableModel>> {
    use crate::infrastructure::paths::find_models_dir_client;

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
/// use voicevox_cli::infrastructure::voicevox::has_available_models;
///
/// if has_available_models() {
///     println!("Models are available");
/// } else {
///     println!("Please download models first");
/// }
/// ```
#[must_use]
pub fn has_available_models() -> bool {
    use crate::infrastructure::paths::find_models_dir_client;

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
            (file_type.is_file() && is_vvm_path(&path))
                || (file_type.is_dir() && has_any_vvm_file(&path))
        })
    })
}

fn find_vvm_files(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    collect_vvm_files(dir)
}

fn collect_vvm_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| anyhow!("Failed to read directory {}: {e}", dir.display()))?;

    entries
        .into_iter()
        .try_fold(Vec::new(), |mut files, entry_result| {
            let entry = entry_result
                .map_err(|e| anyhow!("Failed to read entry in {}: {e}", dir.display()))?;
            let file_type = entry
                .file_type()
                .map_err(|e| anyhow!("Failed to inspect entry in {}: {e}", dir.display()))?;
            let path = entry.path();

            if file_type.is_file() && is_vvm_path(&path) {
                files.push(path);
                return Ok(files);
            }

            if file_type.is_dir() {
                files.extend(collect_vvm_files(&path)?);
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

/// Build style-to-model mapping by scanning all available models dynamically
///
/// # Errors
///
/// Returns an error if model directory scanning fails or model metadata extraction fails.
pub fn build_style_to_model_map_async(
    core: &crate::infrastructure::core::VoicevoxCore,
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
    core: &crate::infrastructure::core::VoicevoxCore,
    mut progress_callback: F,
) -> Result<StyleModelMapBuildResult>
where
    F: FnMut(usize, usize, &str),
{
    use crate::infrastructure::core::CoreSynthesis;
    use std::collections::{HashMap, HashSet};

    let mut style_map = HashMap::new();
    let models_dir = crate::infrastructure::paths::find_models_dir()?;

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

        if let Err(error) = core.load_specific_model(*model_id) {
            crate::infrastructure::logging::warn(&format!(
                "Failed to load model {model_id} ({model_filename}): {error}"
            ));
            continue;
        }

        let Ok(current_speakers) = core.get_speakers() else {
            crate::infrastructure::logging::warn(&format!(
                "Failed to read speakers after loading model {model_id} ({model_filename})"
            ));
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
        .filter_map(
            |(model_id, path)| match core.load_specific_model(*model_id) {
                Ok(()) => Some(path),
                Err(error) => {
                    crate::infrastructure::logging::warn(&format!(
                        "Failed to load model {model_id} ({}): {error}",
                        path.display()
                    ));
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    let all_speakers = match core.get_speakers() {
        Ok(speakers) => speakers,
        Err(error) => {
            for path in &loaded_model_paths {
                unload_model_quietly(core, path);
            }
            return Err(error);
        }
    };

    for path in loaded_model_paths {
        unload_model_quietly(core, path);
    }

    let mut available_models = available_models_from_entries(model_entries);
    populate_model_speakers(&mut available_models, &all_speakers, &style_map);
    sort_models_by_id(&mut available_models);

    Ok((style_map, all_speakers, available_models))
}

#[cfg(test)]
mod tests {
    use super::{AvailableModel, Speaker, SpeakerList, Style, StyleList, populate_model_speakers};
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn populate_model_speakers_groups_styles_by_model() {
        let mut models = vec![
            AvailableModel {
                model_id: 1,
                file_path: PathBuf::from("1.vvm"),
                speakers: SpeakerList::new(),
            },
            AvailableModel {
                model_id: 2,
                file_path: PathBuf::from("2.vvm"),
                speakers: SpeakerList::new(),
            },
        ];
        let speakers = vec![Speaker {
            name: "speaker".into(),
            speaker_uuid: "uuid".into(),
            styles: [
                Style {
                    name: "style-10".into(),
                    id: 10,
                    style_type: None,
                },
                Style {
                    name: "style-20".into(),
                    id: 20,
                    style_type: None,
                },
            ]
            .into_iter()
            .collect::<StyleList>(),
            version: "1".into(),
        }];
        let style_to_model_map = HashMap::from([(10, 1), (20, 2)]);

        populate_model_speakers(&mut models, &speakers, &style_to_model_map);

        assert_eq!(models[0].speakers.len(), 1);
        assert_eq!(models[0].speakers[0].styles.len(), 1);
        assert_eq!(models[0].speakers[0].styles[0].id, 10);
        assert_eq!(models[1].speakers.len(), 1);
        assert_eq!(models[1].speakers[0].styles[0].id, 20);
    }
}
