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

// VVMファイルを自動検出する
pub fn scan_available_models() -> Result<Vec<AvailableModel>> {
    use crate::paths::find_models_dir_client;
    
    let models_dir = find_models_dir_client()?;
    let mut available_models = Vec::new();
    
    // VVMファイルを検索（再帰的）
    let vvm_files = find_vvm_files(&models_dir)?;
    
    for vvm_file in vvm_files {
        if let Some(model_id) = extract_model_id_from_path(&vvm_file) {
            available_models.push(AvailableModel {
                model_id,
                file_path: vvm_file,
                speakers: Vec::new(), // 実際のスピーカー情報は後で動的に取得
            });
        }
    }
    
    // モデルIDでソート
    available_models.sort_by_key(|m| m.model_id);
    
    Ok(available_models)
}

// VVMファイルを再帰的に検索
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
            // 再帰的に検索
            vvm_files.extend(find_vvm_files(&path)?);
        }
    }
    
    Ok(vvm_files)
}

// VVMファイルパスからモデルIDを抽出（例: "3.vvm" -> 3）
fn extract_model_id_from_path(path: &PathBuf) -> Option<u32> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .and_then(|stem| stem.parse::<u32>().ok())
}

// 動的音声解決システム
pub fn resolve_voice_dynamic(voice_input: &str) -> Result<(u32, String)> {
    // 音声一覧表示の特別なケース
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
    
    // 直接的な数値指定をサポート
    if let Ok(style_id) = voice_input.parse::<u32>() {
        return Ok((style_id, format!("Style ID {}", style_id)));
    }
    
    // 高度な音声名解決（VOICEVOX Core統合）
    resolve_voice_with_core_integration(voice_input)
}


// 利用可能なモデルから動的に音声を検索
fn try_resolve_from_available_models(voice_input: &str) -> Result<(u32, String)> {
    // まず利用可能なモデルをスキャン
    let available_models = scan_available_models().map_err(|e| {
        anyhow!("Failed to scan available models: {}. Use --speaker-id for direct ID specification.", e)
    })?;
    
    if available_models.is_empty() {
        return Err(anyhow!(
            "No voice models available. Please download models first or use --speaker-id for direct ID specification."
        ));
    }
    
    // 音声名がモデル番号と一致するかチェック
    if let Ok(model_id) = voice_input.parse::<u32>() {
        if available_models.iter().any(|m| m.model_id == model_id) {
            return Ok((model_id, format!("Model {} (Default Style)", model_id)));
        }
    }
    
    // 一般的なパターンマッチング（動的）
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

// モデルIDから利用可能なスタイルを取得（VOICEVOX Core統合版）
pub fn get_styles_for_model_from_core(model_id: u32) -> Result<Vec<Style>> {
    // VOICEVOX Coreから直接スピーカー情報を取得
    use crate::core::VoicevoxCore;
    
    match VoicevoxCore::new() {
        Ok(core) => {
            // コアが初期化できた場合、スピーカー情報を取得
            match core.get_speakers() {
                Ok(speakers) => {
                    let mut styles = Vec::new();
                    for speaker in speakers {
                        // このモデルに関連するスタイルを抽出
                        for style in speaker.styles {
                            // ヒューリスティック：style.idがmodel_idの範囲内かチェック
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
            // VOICEVOX Coreが利用できない場合は空のベクターを返す
            Ok(Vec::new())
        }
    }
}

// 高度な音声名解決（VOICEVOX Core統合版）
pub fn resolve_voice_with_core_integration(voice_input: &str) -> Result<(u32, String)> {
    // まず基本的な解決を試行
    match try_resolve_from_available_models(voice_input) {
        Ok(result) => Ok(result),
        Err(_) => {
            // 失敗した場合、VOICEVOX Coreから詳細情報を取得
            use crate::core::VoicevoxCore;
            
            if let Ok(core) = VoicevoxCore::new() {
                if let Ok(speakers) = core.get_speakers() {
                    // スピーカー名での検索
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
            
            // 最終的に失敗した場合
            Err(anyhow!(
                "Voice '{}' not found. Use --list-speakers to see all available voices.",
                voice_input
            ))
        }
    }
}

// 音声IDから必要なVVMモデル番号を取得（完全動的版）
pub fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    // 完全に動的な検出：利用可能なモデルから推定
    if let Ok(available_models) = scan_available_models() {
        // voice_idに最も近いmodel_idを検索
        available_models
            .iter()
            .find(|model| {
                // 一般的なパターン：voice_id がmodel_idと同じか、近い値
                model.model_id == voice_id || 
                (voice_id >= model.model_id * 10 && voice_id < (model.model_id + 1) * 10)
            })
            .map(|model| model.model_id)
            .or_else(|| {
                // フォールバック：最初の利用可能なモデル
                available_models.first().map(|model| model.model_id)
            })
    } else {
        // モデルスキャンに失敗した場合のフォールバック
        None
    }
}


