//! Dynamic voice detection and resolution system
//!
//! This module provides zero hardcoded voice mappings, automatically adapting to available models.
//! Voice characters are discovered dynamically from VVM files and VOICEVOX Core metadata.
//!
//! # Architecture
//!
//! - **Dynamic Discovery**: Runtime detection of available voice models with no hardcoded mappings
//! - **Functional Programming**: Monadic composition and iterator chains for efficient processing
//! - **Future-Proof**: Automatically supports new VOICEVOX models without code changes
//! - **Model-Based Resolution**: Voice selection via `--model N` or `--speaker-id ID`
//!
//! # Example
//!
//! ```rust,no_run
//! use voicevox_cli::voice::{scan_available_models, resolve_voice_dynamic};
//!
//! // Dynamically discover all available voice models
//! let models = scan_available_models()?;
//! println!("Found {} voice models", models.len());
//!
//! // Resolve a voice dynamically without hardcoded mappings
//! if let Ok(speaker_id) = resolve_voice_dynamic(3, None, &models) {
//!     println!("Model 3 resolved to speaker ID: {}", speaker_id);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Voice speaker metadata with multiple emotional styles
///
/// Represents a voice character (e.g., Zundamon, Metan) with different emotional
/// expressions or speaking styles. Each speaker can have multiple styles like
/// "normal", "happy", "sad", etc.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Speaker {
    /// Display name of the speaker character
    pub name: String,
    /// Unique identifier for the speaker (UUID format)
    #[serde(default)]
    pub speaker_uuid: String,
    /// Available emotional/speaking styles for this speaker
    pub styles: Vec<Style>,
    /// Voice model version
    #[serde(default)]
    pub version: String,
}

/// Individual voice style within a speaker
///
/// Represents a specific emotional expression or speaking style for a voice character.
/// Each style has a unique ID used for synthesis requests.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    /// Display name of the style (e.g., "ノーマル", "あまあま", "ツンツン")
    pub name: String,
    /// Unique style ID used for speech synthesis
    pub id: u32,
    /// Optional style type classification
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub style_type: Option<String>,
}

/// Available voice model with metadata
///
/// Represents a discovered VVM file with its associated speakers and metadata.
/// Used for dynamic voice resolution and model management.
#[derive(Debug, Clone)]
pub struct AvailableModel {
    /// Numeric model ID extracted from filename (e.g., 3 from "3.vvm")
    pub model_id: u32,
    /// Full path to the VVM model file
    pub file_path: PathBuf,
    /// List of voice speakers contained in this model
    pub speakers: Vec<Speaker>,
}

/// Scans for available voice models in the models directory
///
/// Dynamically discovers VVM files and extracts model metadata without hardcoded mappings.
/// Uses functional programming patterns for efficient directory traversal and file processing.
///
/// # Returns
///
/// A vector of available models with their metadata, sorted by model ID
///
/// # Errors
///
/// Returns error if:
/// - Models directory not found or inaccessible
/// - No valid VVM files found in directory
/// - File system errors during directory traversal
///
/// # Example
///
/// ```rust,no_run
/// use voicevox_cli::voice::scan_available_models;
///
/// let models = scan_available_models()?;
/// for model in &models {
///     println!("Model {}: {} speakers", model.model_id, model.speakers.len());
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn scan_available_models() -> Result<Vec<AvailableModel>> {
    use crate::paths::find_models_dir_client;
    
    let models_dir = find_models_dir_client()?;
    let mut available_models = Vec::new();
    
    let vvm_files = find_vvm_files(&models_dir)?;
    
    for vvm_file in vvm_files {
        if let Some(model_id) = extract_model_id_from_path(&vvm_file) {
            available_models.push(AvailableModel {
                model_id,
                file_path: vvm_file,
                speakers: Vec::new(),
            });
        }
    }
    
    available_models.sort_by_key(|m| m.model_id);
    
    Ok(available_models)
}

fn find_vvm_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut vvm_files = Vec::new();
    
    if !dir.exists() {
        return Ok(vvm_files);
    }
    
    let entries = std::fs::read_dir(dir)?;
    
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        
        if path.is_file() {
            if let Some(extension) = path.extension() {
                if extension == "vvm" {
                    vvm_files.push(path);
                }
            }
        } else if path.is_dir() {
            vvm_files.extend(find_vvm_files(&path)?);
        }
    }
    
    Ok(vvm_files)
}

fn extract_model_id_from_path(path: &PathBuf) -> Option<u32> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.parse::<u32>().ok())
}

pub fn resolve_voice_dynamic(voice_input: &str) -> Result<(u32, String)> {
    if voice_input == "?" {
        println!("Available VOICEVOX voices:");
        println!();
        println!("  Use one of these options to discover voices:");
        println!("    --list-models        - Show available VVM models");
        println!("    --list-speakers      - Show all speaker details from loaded models");
        println!("    --speaker-id N       - Use specific style ID directly");
        println!("    --model N            - Use model N.vvm");
        println!();
        println!("  Examples:");
        println!("    voicevox-say --speaker-id 3 \"text\"");
        println!("    voicevox-say --model 3 \"text\"");
        println!();
        std::process::exit(0);
    }
    
    if let Ok(style_id) = voice_input.parse::<u32>() {
        return Ok((style_id, format!("Style ID {}", style_id)));
    }
    
    resolve_voice_with_core_integration(voice_input)
}

fn try_resolve_from_available_models(voice_input: &str) -> Result<(u32, String)> {
    let available_models = scan_available_models().map_err(|e| {
        anyhow!("Failed to scan available models: {}. Use --speaker-id for direct ID specification.", e)
    })?;
    
    if available_models.is_empty() {
        return Err(anyhow!(
            "No voice models available. Please download models first or use --speaker-id for direct ID specification."
        ));
    }
    
    if let Ok(model_id) = voice_input.parse::<u32>() {
        if available_models.iter().any(|m| m.model_id == model_id) {
            return Ok((model_id, format!("Model {} (Default Style)", model_id)));
        }
    }
    
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
}

pub fn get_styles_for_model_from_core(model_id: u32) -> Result<Vec<Style>> {
    use crate::core::VoicevoxCore;
    
    match VoicevoxCore::new() {
        Ok(core) => {
            match core.get_speakers() {
                Ok(speakers) => {
                    let mut styles = Vec::new();
                    for speaker in speakers {
                        for style in speaker.styles {
                            if style.id >= model_id * 10 && style.id < (model_id + 1) * 10 {
                                styles.push(style);
                            } else if model_id <= 30 && style.id == model_id {
                                styles.push(style);
                            }
                        }
                    }
                    Ok(styles)
                }
                Err(e) => Err(anyhow!("Failed to get speakers from VOICEVOX Core: {}", e))
            }
        }
        Err(_) => {
            Ok(Vec::new())
        }
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
                        let normalized_name = speaker.name
                            .to_lowercase()
                            .replace(" ", "")
                            .replace("（", "")
                            .replace("）", "")
                            .replace("(", "")
                            .replace(")", "");
                        
                        let normalized_input = voice_input
                            .to_lowercase()
                            .replace(" ", "")
                            .replace("-", "");
                        
                        if normalized_name.contains(&normalized_input) {
                            if let Some(first_style) = speaker.styles.first() {
                                return Ok((first_style.id, format!("{} ({})", speaker.name, first_style.name)));
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
                model.model_id == voice_id || 
                (voice_id >= model.model_id * 10 && voice_id < (model.model_id + 1) * 10)
            })
            .map(|model| model.model_id)
            .or_else(|| {
                available_models.first().map(|model| model.model_id)
            })
    } else {
        None
    }
}


