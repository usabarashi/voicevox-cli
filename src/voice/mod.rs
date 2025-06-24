use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Speaker {
    pub name: String,
    #[serde(default)]
    pub speaker_uuid: String,
    pub styles: Vec<Style>,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    pub name: String,
    pub id: u32,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub style_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AvailableModel {
    pub model_id: u32,
    pub file_path: PathBuf,
    pub speakers: Vec<Speaker>,
}

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
    
    // Support direct numeric specification
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
    
    // Check if voice name matches model number
    if let Ok(model_id) = voice_input.parse::<u32>() {
        if available_models.iter().any(|m| m.model_id == model_id) {
            return Ok((model_id, format!("Model {} (Default Style)", model_id)));
        }
    }
    
    // General pattern matching (dynamic)
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

// Get available styles from model ID (VOICEVOX Core integration version)
pub fn get_styles_for_model_from_core(model_id: u32) -> Result<Vec<Style>> {
    // Get speaker information directly from VOICEVOX Core
    use crate::core::VoicevoxCore;
    
    match VoicevoxCore::new() {
        Ok(core) => {
            // Get speaker information if core initialization succeeds
            match core.get_speakers() {
                Ok(speakers) => {
                    let mut styles = Vec::new();
                    for speaker in speakers {
                        // Extract styles related to this model
                        for style in speaker.styles {
                            // Heuristic: Check if style.id is within model_id range
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
            // Return empty vector if VOICEVOX Core is unavailable
            Ok(Vec::new())
        }
    }
}

// Advanced voice name resolution (VOICEVOX Core integration version)
pub fn resolve_voice_with_core_integration(voice_input: &str) -> Result<(u32, String)> {
    // First attempt basic resolution
    match try_resolve_from_available_models(voice_input) {
        Ok(result) => Ok(result),
        Err(_) => {
            // If failed, get detailed information from VOICEVOX Core
            use crate::core::VoicevoxCore;
            
            if let Ok(core) = VoicevoxCore::new() {
                if let Ok(speakers) = core.get_speakers() {
                    // Search by speaker name
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
            
            // If ultimately failed
            Err(anyhow!(
                "Voice '{}' not found. Use --list-speakers to see all available voices.",
                voice_input
            ))
        }
    }
}

// Get required VVM model number from voice ID (fully dynamic version)
pub fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    // Fully dynamic detection: infer from available models
    if let Ok(available_models) = scan_available_models() {
        // Search for model_id closest to voice_id
        available_models
            .iter()
            .find(|model| {
                // General pattern: voice_id same as model_id or close value
                model.model_id == voice_id || 
                (voice_id >= model.model_id * 10 && voice_id < (model.model_id + 1) * 10)
            })
            .map(|model| model.model_id)
            .or_else(|| {
                // Fallback: first available model
                available_models.first().map(|model| model.model_id)
            })
    } else {
        // Fallback when model scanning fails
        None
    }
}


