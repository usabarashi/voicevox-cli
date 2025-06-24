use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
        println!("  Basic voices:");
        println!("    zundamon     (ID: 3)  - ずんだもん (ノーマル)");
        println!("    metan        (ID: 2)  - 四国めたん (ノーマル)");
        println!("    tsumugi      (ID: 8)  - 春日部つむぎ (ノーマル)");
        println!();
        println!("  For more voices, use:");
        println!("    --list-models        - Show available VVM models");
        println!("    --list-speakers      - Show all speaker details");
        println!("    --speaker-id N       - Use specific style ID");
        println!();
        std::process::exit(0);
    }
    
    // 直接的な数値指定をサポート
    if let Ok(style_id) = voice_input.parse::<u32>() {
        return Ok((style_id, format!("Style ID {}", style_id)));
    }
    
    // 基本的な音声名マッピング（最小限）
    let basic_voices = get_basic_voice_mapping();
    
    if let Some((style_id, description)) = basic_voices.get(voice_input) {
        Ok((*style_id, description.to_string()))
    } else {
        // 利用可能なモデルから動的に検索
        try_resolve_from_available_models(voice_input)
    }
}

// 基本的な音声名マッピング（最小限）
fn get_basic_voice_mapping() -> HashMap<&'static str, (u32, &'static str)> {
    let mut voices = HashMap::new();
    
    // 最も一般的な音声のみ
    voices.insert("zundamon", (3, "ずんだもん (ノーマル)"));
    voices.insert("metan", (2, "四国めたん (ノーマル)"));
    voices.insert("tsumugi", (8, "春日部つむぎ (ノーマル)"));
    voices.insert("default", (3, "ずんだもん (ノーマル)"));
    
    voices
}

// 利用可能なモデルから動的に音声を検索
fn try_resolve_from_available_models(voice_input: &str) -> Result<(u32, String)> {
    // 将来的にVOICEVOX Coreからスピーカー情報を動的に取得する予定
    // 現時点では基本的なエラーメッセージを返す
    Err(anyhow!(
        "Voice '{}' not found. Use --speaker-id for direct ID specification, or --list-models to see available models.",
        voice_input
    ))
}

// モデルIDから利用可能なスタイルを取得（将来の実装用）
#[allow(dead_code)]
fn get_styles_for_model(_model_id: u32) -> Result<Vec<Style>> {
    // この関数は将来、VOICEVOX Coreから動的にスタイル情報を取得するために使用される
    // 現時点では空のベクターを返す
    Ok(Vec::new())
}

// 音声IDから必要なVVMモデル番号を取得（簡素化版）
pub fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    // 基本的なヒューリスティック：voice_id がモデル番号と一致することが多い
    // より正確な情報は VOICEVOX Core から動的に取得する
    if voice_id <= 30 {
        Some(voice_id) // 簡単なマッピング
    } else {
        None // 不明な場合は動的検出に任せる
    }
}


