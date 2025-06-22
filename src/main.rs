use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::ptr;

// Use bindgen-generated bindings if available
#[cfg(feature = "use_bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Define constants for result codes (bindgen may not expose them properly)
#[cfg(feature = "use_bindgen")]
pub const VOICEVOX_RESULT_OK: i32 = 0;

// If bindgen fails, provide manual bindings (simplified)
#[cfg(not(feature = "use_bindgen"))]
mod manual_bindings {
    use libc::{c_char, c_int, c_uchar, c_uint, uintptr_t};

    pub const VOICEVOX_RESULT_OK: c_int = 0;
    pub type VoicevoxStyleId = c_uint;

    // Acceleration mode enum for macOS CPU-only processing
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub enum VoicevoxAccelerationMode {
        Auto = 0,
        Cpu = 1,
        Gpu = 2,
    }

    // Initialize options structure for CPU-only mode
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct VoicevoxInitializeOptions {
        pub acceleration_mode: VoicevoxAccelerationMode,
        pub cpu_num_threads: u16,
    }

    // Opaque types
    pub enum VoicevoxSynthesizer {}
    pub enum VoicevoxOnnxruntime {}
    pub enum OpenJtalkRc {}
    pub enum VoicevoxLoadOnnxruntimeOptions {}
    pub enum VoicevoxTtsOptions {}
    pub enum VoicevoxSynthesisOptions {}
    pub enum VoicevoxVoiceModelFile {}

    extern "C" {
        // Core initialization functions
        pub fn voicevox_make_default_load_onnxruntime_options(
        ) -> *const VoicevoxLoadOnnxruntimeOptions;
        pub fn voicevox_onnxruntime_load_once(
            options: *const VoicevoxLoadOnnxruntimeOptions,
            onnxruntime: *mut *const VoicevoxOnnxruntime,
        ) -> c_int;

        pub fn voicevox_open_jtalk_rc_new(
            open_jtalk_dic_dir: *const c_char,
            open_jtalk_rc: *mut *mut OpenJtalkRc,
        ) -> c_int;

        // Initialize options with CPU-only mode
        pub fn voicevox_synthesizer_new(
            onnxruntime: *const VoicevoxOnnxruntime,
            open_jtalk_rc: *mut OpenJtalkRc,
            options: VoicevoxInitializeOptions,
            synthesizer: *mut *mut VoicevoxSynthesizer,
        ) -> c_int;

        // TTS functions
        pub fn voicevox_make_default_tts_options() -> *const VoicevoxTtsOptions;
        pub fn voicevox_synthesizer_tts(
            synthesizer: *mut VoicevoxSynthesizer,
            text: *const c_char,
            style_id: VoicevoxStyleId,
            options: *const VoicevoxTtsOptions,
            wav_length: *mut uintptr_t,
            wav: *mut *mut c_uchar,
        ) -> c_int;

        // Metadata functions
        pub fn voicevox_synthesizer_create_metas_json(
            synthesizer: *mut VoicevoxSynthesizer,
        ) -> *mut c_char;

        // Model loading functions
        pub fn voicevox_synthesizer_load_voice_model(
            synthesizer: *const VoicevoxSynthesizer,
            model: *const VoicevoxVoiceModelFile,
        ) -> c_int;

        pub fn voicevox_voice_model_file_open(
            path: *const c_char,
            model: *mut *mut VoicevoxVoiceModelFile,
        ) -> c_int;

        pub fn voicevox_voice_model_file_delete(model: *mut VoicevoxVoiceModelFile);

        // Cleanup functions
        pub fn voicevox_wav_free(wav: *mut c_uchar);
        pub fn voicevox_json_free(json: *mut c_char);
        pub fn voicevox_synthesizer_delete(synthesizer: *mut VoicevoxSynthesizer);
        pub fn voicevox_open_jtalk_rc_delete(open_jtalk_rc: *mut OpenJtalkRc);
    }
}

#[cfg(not(feature = "use_bindgen"))]
use manual_bindings::*;

#[derive(Debug, Serialize, Deserialize)]
struct Speaker {
    name: String,
    #[serde(default)]
    speaker_uuid: String,
    styles: Vec<Style>,
    #[serde(default)]
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Style {
    name: String,
    id: u32,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    style_type: Option<String>,
}

#[derive(Debug)]
struct VoicevoxCore {
    synthesizer: *mut VoicevoxSynthesizer,
    _open_jtalk_rc: *mut OpenJtalkRc,
}

unsafe impl Send for VoicevoxCore {}
unsafe impl Sync for VoicevoxCore {}

impl VoicevoxCore {
    fn new() -> Result<Self> {
        unsafe {
            // Load ONNX Runtime first
            let load_options = voicevox_make_default_load_onnxruntime_options();
            let mut onnxruntime: *const VoicevoxOnnxruntime = ptr::null();

            let result = voicevox_onnxruntime_load_once(load_options, &mut onnxruntime);
            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!(
                    "ONNX Runtime initialization failed: code {}",
                    result
                ));
            }

            // Initialize OpenJTalk
            let dict_path = find_openjtalk_dict()?;
            let dict_cstr = CString::new(dict_path)?;
            let mut open_jtalk_rc: *mut OpenJtalkRc = ptr::null_mut();

            let result = voicevox_open_jtalk_rc_new(dict_cstr.as_ptr(), &mut open_jtalk_rc);
            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!(
                    "OpenJTalk RC initialization failed: code {}",
                    result
                ));
            }

            // Create synthesizer with CPU-only mode for macOS
            #[cfg(target_os = "macos")]
            {
                println!("🖥️  Initializing VOICEVOX Core in CPU-only mode for macOS...");

                // Create CPU-only initialization options structure
                let init_options = VoicevoxInitializeOptions {
                    acceleration_mode: VoicevoxAccelerationMode::Cpu, // Force CPU mode, no GPU testing
                    cpu_num_threads: 0, // Use default number of CPU threads (0 = auto-detect)
                };

                let mut synthesizer: *mut VoicevoxSynthesizer = ptr::null_mut();
                let result = voicevox_synthesizer_new(
                    onnxruntime,
                    open_jtalk_rc,
                    init_options,
                    &mut synthesizer,
                );

                if result != VOICEVOX_RESULT_OK {
                    voicevox_open_jtalk_rc_delete(open_jtalk_rc);
                    return Err(anyhow!("Synthesizer creation failed: code {}", result));
                }

                println!("✅ Initialization completed successfully");

                // Skip loading any default models - load only what's needed later
                // This makes startup much faster!

                Ok(VoicevoxCore {
                    synthesizer,
                    _open_jtalk_rc: open_jtalk_rc,
                })
            }

            // Fallback for non-macOS platforms - also use CPU-only mode
            #[cfg(not(target_os = "macos"))]
            {
                println!("🖥️  Initializing VOICEVOX Core in CPU-only mode...");

                // Create CPU-only initialization options structure
                let init_options = VoicevoxInitializeOptions {
                    acceleration_mode: VoicevoxAccelerationMode::Cpu, // Force CPU mode, no GPU testing
                    cpu_num_threads: 0, // Use default number of CPU threads (0 = auto-detect)
                };

                let mut synthesizer: *mut VoicevoxSynthesizer = ptr::null_mut();
                let result = voicevox_synthesizer_new(
                    onnxruntime,
                    open_jtalk_rc,
                    init_options,
                    &mut synthesizer,
                );

                if result != VOICEVOX_RESULT_OK {
                    voicevox_open_jtalk_rc_delete(open_jtalk_rc);
                    return Err(anyhow!("Synthesizer creation failed: code {}", result));
                }

                println!("✅ Initialization completed successfully");

                // Do not load any models by default for fastest startup
                // Models will be loaded on demand based on the requested voice
                println!("🚀 Core initialized - models will be loaded on demand");

                Ok(VoicevoxCore {
                    synthesizer,
                    _open_jtalk_rc: open_jtalk_rc,
                })
            }
        }
    }

    // Helper function to get the model number for a given voice/style ID

    fn load_default_models(synthesizer: *mut VoicevoxSynthesizer) -> Result<()> {
        // Load only essential models for faster startup
        // Priority: ずんだもん (3.vvm), 四国めたん (2.vvm), 春日部つむぎ (8.vvm)
        let default_models = ["3.vvm", "2.vvm", "8.vvm"];

        let models_dir = find_models_dir()?;

        println!("📦 Loading default VVM models for faster startup...");

        let mut loaded_count = 0;
        for model_name in &default_models {
            let model_path = models_dir.join(model_name);
            if model_path.exists() {
                if let Some(path_str) = model_path.to_str() {
                    if let Ok(path_cstr) = CString::new(path_str) {
                        unsafe {
                            let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                            let result =
                                voicevox_voice_model_file_open(path_cstr.as_ptr(), &mut model);
                            if result == VOICEVOX_RESULT_OK {
                                let load_result =
                                    voicevox_synthesizer_load_voice_model(synthesizer, model);
                                if load_result == VOICEVOX_RESULT_OK {
                                    loaded_count += 1;
                                    println!("  ✅ Loaded: {}", model_name);
                                } else {
                                    println!(
                                        "  ⚠️  Failed to load: {} (code: {})",
                                        model_name, load_result
                                    );
                                }
                                voicevox_voice_model_file_delete(model);
                            } else {
                                println!("  ⚠️  Failed to open: {} (code: {})", model_name, result);
                            }
                        }
                    }
                }
            } else {
                println!("  ⚠️  Model not found: {}", model_name);
            }
        }

        if loaded_count > 0 {
            println!("✅ Successfully loaded {} default VVM models", loaded_count);
        } else {
            println!("⚠️  No default VVM models were loaded");
        }

        Ok(())
    }

    fn load_specific_model(&self, model_name: &str) -> Result<()> {
        let models_dir = find_models_dir()?;
        let model_path = models_dir.join(format!("{}.vvm", model_name));

        if !model_path.exists() {
            return Err(anyhow!("Model not found: {}.vvm", model_name));
        }

        println!("📦 Loading model: {}.vvm", model_name);

        if let Some(path_str) = model_path.to_str() {
            if let Ok(path_cstr) = CString::new(path_str) {
                unsafe {
                    let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                    let result = voicevox_voice_model_file_open(path_cstr.as_ptr(), &mut model);
                    if result == VOICEVOX_RESULT_OK {
                        let load_result =
                            voicevox_synthesizer_load_voice_model(self.synthesizer, model);
                        if load_result == VOICEVOX_RESULT_OK {
                            println!("  ✅ Successfully loaded: {}.vvm", model_name);
                        } else if load_result == 18 {
                            // MODEL_ALREADY_LOADED_ERROR
                            // Model already loaded, this is OK
                            println!("  ℹ️  Model {}.vvm already loaded", model_name);
                        } else {
                            voicevox_voice_model_file_delete(model);
                            return Err(anyhow!(
                                "Failed to load model: {} (code: {})",
                                model_name,
                                load_result
                            ));
                        }
                        voicevox_voice_model_file_delete(model);
                    } else {
                        return Err(anyhow!(
                            "Failed to open model: {} (code: {})",
                            model_name,
                            result
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn load_models(synthesizer: *mut VoicevoxSynthesizer) -> Result<()> {
        // Find the models directory
        let models_dir = find_models_dir()?;

        println!("📦 Loading VVM models from: {}", models_dir.display());

        // Load all VVM files
        let mut loaded_count = 0;
        if let Ok(entries) = std::fs::read_dir(&models_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".vvm") {
                        let model_path = entry.path();
                        if let Some(path_str) = model_path.to_str() {
                            if let Ok(path_cstr) = CString::new(path_str) {
                                unsafe {
                                    // Try to load the model
                                    let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                                    let result = voicevox_voice_model_file_open(
                                        path_cstr.as_ptr(),
                                        &mut model,
                                    );
                                    if result == VOICEVOX_RESULT_OK {
                                        let load_result = voicevox_synthesizer_load_voice_model(
                                            synthesizer,
                                            model,
                                        );
                                        if load_result == VOICEVOX_RESULT_OK {
                                            loaded_count += 1;
                                            println!("  ✅ Loaded: {}", file_name);
                                        } else {
                                            println!(
                                                "  ⚠️  Failed to load: {} (code: {})",
                                                file_name, load_result
                                            );
                                        }
                                        voicevox_voice_model_file_delete(model);
                                    } else {
                                        println!(
                                            "  ⚠️  Failed to open: {} (code: {})",
                                            file_name, result
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if loaded_count > 0 {
            println!("✅ Successfully loaded {} VVM models", loaded_count);
        } else {
            println!("⚠️  No VVM models were loaded");
        }

        Ok(())
    }

    fn synthesize_simple(&self, text: &str, style_id: VoicevoxStyleId) -> Result<Vec<u8>> {
        unsafe {
            let text_cstr = CString::new(text)?;
            let tts_options = voicevox_make_default_tts_options();
            let mut wav_data: *mut u8 = ptr::null_mut();
            let mut wav_length: usize = 0;

            let result = voicevox_synthesizer_tts(
                self.synthesizer,
                text_cstr.as_ptr(),
                style_id,
                tts_options,
                &mut wav_length,
                &mut wav_data,
            );

            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!("Speech synthesis failed: code {}", result));
            }

            if wav_data.is_null() || wav_length == 0 {
                return Err(anyhow!("Audio data was not generated"));
            }

            let wav_vec = std::slice::from_raw_parts(wav_data, wav_length).to_vec();
            voicevox_wav_free(wav_data);
            Ok(wav_vec)
        }
    }

    fn synthesize_streaming(&self, text: &str, style_id: VoicevoxStyleId) -> Result<()> {
        self.synthesize_streaming_with_config(text, style_id, 100, None)
    }

    fn synthesize_streaming_with_config(
        &self,
        text: &str,
        style_id: VoicevoxStyleId,
        delay_ms: u64,
        chunk_size: Option<usize>,
    ) -> Result<()> {
        // テキストを適切なサイズに分割
        let sentences = if let Some(size) = chunk_size {
            split_text_by_size(text, size)
        } else {
            split_sentences(text)
        };

        // オーディオストリームとシンクを初期化
        let (_stream, stream_handle) = OutputStream::try_default()
            .map_err(|e| anyhow!("Failed to create audio stream: {}", e))?;
        let sink = Sink::try_new(&stream_handle)
            .map_err(|e| anyhow!("Failed to create audio sink: {}", e))?;

        println!(
            "🎵 Starting streaming synthesis for {} segments...",
            sentences.len()
        );
        if chunk_size.is_some() {
            println!(
                "   📏 Using character-based chunking (max {} chars per chunk)",
                chunk_size.unwrap()
            );
        } else {
            println!("   � Using sentence-based chunking");
        }
        println!("   ⏱️  Delay between segments: {}ms", delay_ms);

        let start_time = std::time::Instant::now();
        let mut total_synthesis_time = std::time::Duration::ZERO;

        // 各セグメントを順次合成・再生
        for (i, segment) in sentences.iter().enumerate() {
            if segment.trim().is_empty() {
                continue;
            }

            let segment_display = if segment.len() > 30 {
                format!("{}...", &segment[..30])
            } else {
                segment.clone()
            };

            println!(
                "  🔊 [{}/{}] Processing: \"{}\"",
                i + 1,
                sentences.len(),
                segment_display
            );

            let synthesis_start = std::time::Instant::now();
            // 音声合成
            let wav_data = self.synthesize_simple(segment, style_id)?;
            let synthesis_time = synthesis_start.elapsed();
            total_synthesis_time += synthesis_time;

            // WAVデータを音声デコーダーに変換
            let cursor = Cursor::new(wav_data);
            match Decoder::new(cursor) {
                Ok(source) => {
                    // 音声をキューに追加（ノンブロッキング）
                    sink.append(source);

                    println!("    ⚡ Synthesis: {:?}, Audio queued", synthesis_time);

                    // 設定された間隔で待機
                    if delay_ms > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                }
                Err(e) => {
                    println!("  ⚠️  Failed to decode audio for segment {}: {}", i + 1, e);
                }
            }
        }

        // 全ての音声が再生されるまで待機
        println!("⏳ Waiting for audio playback to complete...");
        sink.sleep_until_end();

        let total_time = start_time.elapsed();
        println!("✅ Streaming synthesis completed!");
        println!(
            "   📊 Total time: {:?}, Synthesis time: {:?}, Efficiency: {:.1}%",
            total_time,
            total_synthesis_time,
            (total_synthesis_time.as_secs_f64() / total_time.as_secs_f64()) * 100.0
        );
        Ok(())
    }

    fn get_speakers(&self) -> Result<Vec<Speaker>> {
        unsafe {
            let metas_json = voicevox_synthesizer_create_metas_json(self.synthesizer);
            if metas_json.is_null() {
                return Err(anyhow!("Failed to get speaker metadata"));
            }

            let metas_str = CStr::from_ptr(metas_json).to_str()?;
            let speakers: Vec<Speaker> = serde_json::from_str(metas_str)
                .map_err(|e| anyhow!("Failed to parse speaker metadata: {}", e))?;

            voicevox_json_free(metas_json);
            Ok(speakers)
        }
    }
}

impl Drop for VoicevoxCore {
    fn drop(&mut self) {
        unsafe {
            if !self.synthesizer.is_null() {
                voicevox_synthesizer_delete(self.synthesizer);
            }
            if !self._open_jtalk_rc.is_null() {
                voicevox_open_jtalk_rc_delete(self._open_jtalk_rc);
            }
        }
    }
}

// Helper function to find VVM models directory
fn find_models_dir() -> Result<PathBuf> {
    // Priority 1: Package installation path (when used as a Nix package)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(pkg_root) = exe_path.parent().and_then(|p| p.parent()) {
            let pkg_models_path = pkg_root.join("share/voicevox/models");
            if pkg_models_path.exists() {
                return Ok(pkg_models_path);
            }
        }
    }

    // Priority 2: Development/workspace paths
    let workspace_root = std::env::current_dir()
        .ok()
        .and_then(|current_dir| {
            current_dir
                .ancestors()
                .find(|a| a.join("voicevox_models").exists())
                .map(|p| p.to_path_buf())
        })
        .unwrap_or_else(|| PathBuf::from("/Users/gen/Documents/usabarashi/mynix"));

    let models_dir = workspace_root.join("voicevox_models/models/vvms");
    if models_dir.exists() {
        Ok(models_dir)
    } else {
        // Priority 3: Environment variable (fallback)
        if let Ok(models_dir) = std::env::var("VOICEVOX_MODELS_DIR") {
            let models_path = PathBuf::from(&models_dir);
            if models_path.exists() {
                return Ok(models_path);
            }
        }
        Err(anyhow!("VVM models directory not found. Checked paths: package path, workspace path, environment VOICEVOX_MODELS_DIR"))
    }
}

// Helper function to check if a directory contains .dic files
fn has_dic_files(dict_path: &PathBuf) -> bool {
    if let Ok(entries) = std::fs::read_dir(dict_path) {
        entries.filter_map(|e| e.ok()).any(|e| {
            if let Some(file_name) = e.file_name().to_str() {
                file_name.ends_with(".dic")
            } else {
                false
            }
        })
    } else {
        false
    }
}

fn find_openjtalk_dict() -> Result<String> {
    // Priority 1: Package installation path (when used as a Nix package)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(pkg_root) = exe_path.parent().and_then(|p| p.parent()) {
            let pkg_dict_path = pkg_root.join("share/voicevox/dict");
            if pkg_dict_path.exists() && has_dic_files(&pkg_dict_path) {
                let path_str = pkg_dict_path.to_string_lossy().to_string();
                println!("Found OpenJTalk dictionary (package): {}", path_str);
                return Ok(path_str);
            }
        }
    }

    // Priority 2: Development/workspace paths
    let workspace_root = std::env::current_dir()
        .ok()
        .and_then(|current_dir| {
            current_dir
                .ancestors()
                .find(|a| a.join("voicevox_models").exists())
                .map(|p| p.to_path_buf())
        })
        .unwrap_or_else(|| PathBuf::from("/Users/gen/Documents/usabarashi/mynix"));

    let possible_dict_paths = vec![
        workspace_root
            .join("voicevox_models/dict/open_jtalk_dic_utf_8-1.11")
            .to_string_lossy()
            .to_string(),
        workspace_root
            .join("dict/open_jtalk_dic_utf_8-1.11")
            .to_string_lossy()
            .to_string(),
        // System fallback paths
        "/opt/homebrew/share/open-jtalk/dic".to_string(),
        "/usr/local/share/open-jtalk/dic".to_string(),
        "/opt/local/share/open-jtalk/dic".to_string(),
        "/usr/share/open-jtalk/dic".to_string(),
        "./dict".to_string(),
    ];

    for path in &possible_dict_paths {
        let dict_path = PathBuf::from(path);
        if dict_path.exists() {
            // Check for .dic files
            if let Ok(entries) = std::fs::read_dir(&dict_path) {
                let has_dic_files = entries.filter_map(|e| e.ok()).any(|e| {
                    if let Some(file_name) = e.file_name().to_str() {
                        file_name.ends_with(".dic")
                    } else {
                        false
                    }
                });

                if has_dic_files {
                    println!("Found OpenJTalk dictionary: {}", path);
                    return Ok(path.to_string());
                }
            }
        }
    }

    // Priority 3: Environment variable (fallback)
    if let Ok(dict_dir) = std::env::var("VOICEVOX_DICT_DIR") {
        let dict_path = PathBuf::from(&dict_dir);
        if dict_path.exists() && has_dic_files(&dict_path) {
            println!("Found OpenJTalk dictionary (env fallback): {}", dict_dir);
            return Ok(dict_dir);
        }
    }

    Err(anyhow!(
        "OpenJTalk dictionary not found.\nChecked paths: {:?}",
        possible_dict_paths
    ))
}

// 音声IDから必要なVVMモデル番号を取得
fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    match voice_id {
        // ずんだもん (3.vvm)
        1 | 3 | 7 => Some(3),
        // 四国めたん (2.vvm)
        2 | 0 | 6 | 4 => Some(2),
        // 春日部つむぎ (8.vvm)
        8 | 83 | 84 => Some(8),
        // 雨晴はう (10.vvm)
        10 | 85 => Some(10),
        // 波音リツ (9.vvm)
        9 | 65 => Some(9),
        // 玄野武宏 (11.vvm)
        11 | 39 | 40 | 41 => Some(11),
        // 白上虎太郎 (12.vvm)
        12 | 32 | 33 => Some(12),
        // 青山龍星 (13.vvm)
        13 | 86 | 87 | 88 | 89 | 90 => Some(13),
        // 冥鳴ひまり (14.vvm)
        14 => Some(14),
        // 九州そら (16.vvm)
        15 | 16 | 17 | 18 | 19 => Some(16),
        // もち子さん (17.vvm)
        20 => Some(17),
        // 剣崎雌雄 (18.vvm)
        21 => Some(18),
        // デフォルトは不明
        _ => None,
    }
}

// 音声名からスタイルIDへのマッピング
fn get_voice_mapping() -> HashMap<&'static str, (u32, &'static str)> {
    let mut voices = HashMap::new();

    // ずんだもん（全モード）
    voices.insert("zundamon", (3, "ずんだもん (ノーマル)"));
    voices.insert("zundamon-normal", (3, "ずんだもん (ノーマル)"));
    voices.insert("zundamon-amama", (1, "ずんだもん (あまあま)"));
    voices.insert("zundamon-tsundere", (7, "ずんだもん (ツンツン)"));
    voices.insert("zundamon-sexy", (5, "ずんだもん (セクシー)"));
    voices.insert("zundamon-whisper", (22, "ずんだもん (ささやき)"));
    voices.insert("zundamon-excited", (38, "ずんだもん (ヘロヘロ)"));

    // 四国めたん（全モード）
    voices.insert("metan", (2, "四国めたん (ノーマル)"));
    voices.insert("metan-normal", (2, "四国めたん (ノーマル)"));
    voices.insert("metan-amama", (0, "四国めたん (あまあま)"));
    voices.insert("metan-tsundere", (6, "四国めたん (ツンツン)"));
    voices.insert("metan-sexy", (4, "四国めたん (セクシー)"));
    voices.insert("metan-whisper", (36, "四国めたん (ささやき)"));
    voices.insert("metan-excited", (37, "四国めたん (ヘロヘロ)"));

    // 春日部つむぎ
    voices.insert("tsumugi", (8, "春日部つむぎ (ノーマル)"));
    voices.insert("tsumugi-normal", (8, "春日部つむぎ (ノーマル)"));

    // 雨晴はう
    voices.insert("hau", (10, "雨晴はう (ノーマル)"));
    voices.insert("hau-normal", (10, "雨晴はう (ノーマル)"));

    // 波音リツ
    voices.insert("ritsu", (9, "波音リツ (ノーマル)"));
    voices.insert("ritsu-normal", (9, "波音リツ (ノーマル)"));

    // 玄野武宏
    voices.insert("takehiro", (11, "玄野武宏 (ノーマル)"));
    voices.insert("takehiro-normal", (11, "玄野武宏 (ノーマル)"));
    voices.insert("takehiro-excited", (39, "玄野武宏 (喜び)"));
    voices.insert("takehiro-tsundere", (40, "玄野武宏 (ツンギレ)"));
    voices.insert("takehiro-sad", (41, "玄野武宏 (悲しみ)"));

    // 白上虎太郎
    voices.insert("kohtaro", (12, "白上虎太郎 (ふつう)"));
    voices.insert("kohtaro-normal", (12, "白上虎太郎 (ふつう)"));
    voices.insert("kohtaro-excited", (32, "白上虎太郎 (わーい)"));
    voices.insert("kohtaro-angry", (33, "白上虎太郎 (びくびく)"));

    // 青山龍星
    voices.insert("ryusei", (13, "青山龍星 (ノーマル)"));
    voices.insert("ryusei-normal", (13, "青山龍星 (ノーマル)"));
    voices.insert("ryusei-excited", (86, "青山龍星 (熱血)"));
    voices.insert("ryusei-cool", (87, "青山龍星 (不機嫌)"));
    voices.insert("ryusei-sad", (88, "青山龍星 (喜び)"));
    voices.insert("ryusei-surprised", (89, "青山龍星 (しっとり)"));
    voices.insert("ryusei-whisper", (90, "青山龍星 (かなしみ)"));

    // 冥鳴ひまり
    voices.insert("himari", (14, "冥鳴ひまり (ノーマル)"));
    voices.insert("himari-normal", (14, "冥鳴ひまり (ノーマル)"));

    // 九州そら
    voices.insert("sora", (16, "九州そら (ノーマル)"));
    voices.insert("sora-normal", (16, "九州そら (ノーマル)"));
    voices.insert("sora-amama", (15, "九州そら (あまあま)"));
    voices.insert("sora-tsundere", (18, "九州そら (ツンツン)"));
    voices.insert("sora-sexy", (17, "九州そら (セクシー)"));
    voices.insert("sora-whisper", (19, "九州そら (ささやき)"));

    // もち子さん
    voices.insert("mochiko", (20, "もち子さん (ノーマル)"));
    voices.insert("mochiko-normal", (20, "もち子さん (ノーマル)"));

    // 剣崎雌雄
    voices.insert("menou", (21, "剣崎雌雄 (ノーマル)"));
    voices.insert("menou-normal", (21, "剣崎雌雄 (ノーマル)"));

    // デフォルトエイリアス
    voices.insert("default", (3, "ずんだもん (ノーマル)"));

    voices
}

fn resolve_voice_name_with_core(voice_name: &str, core: &VoicevoxCore) -> Result<(u32, String)> {
    let voices = get_voice_mapping();

    // 音声一覧表示の特別なケース
    if voice_name == "?" {
        println!("🎭 Available VOICEVOX voices:");
        println!();

        // キャラクター別にグループ化して表示
        println!("  📝 ずんだもん:");
        println!("    zundamon, zundamon-normal    (ID: 3)  - ずんだもん (ノーマル)");
        println!("    zundamon-amama              (ID: 1)  - ずんだもん (あまあま)");
        println!("    zundamon-tsundere           (ID: 7)  - ずんだもん (ツンツン)");
        println!("    zundamon-sexy               (ID: 5)  - ずんだもん (セクシー)");
        println!("    zundamon-whisper            (ID: 22) - ずんだもん (ささやき)");
        println!("    zundamon-excited            (ID: 38) - ずんだもん (ヘロヘロ)");
        println!();

        println!("  🍊 四国めたん:");
        println!("    metan, metan-normal         (ID: 2)  - 四国めたん (ノーマル)");
        println!("    metan-amama                 (ID: 0)  - 四国めたん (あまあま)");
        println!("    metan-tsundere              (ID: 6)  - 四国めたん (ツンツン)");
        println!("    metan-sexy                  (ID: 4)  - 四国めたん (セクシー)");
        println!("    metan-whisper               (ID: 36) - 四国めたん (ささやき)");
        println!("    metan-excited               (ID: 37) - 四国めたん (ヘロヘロ)");
        println!();

        println!("  🌸 その他のキャラクター:");
        println!("    tsumugi                     (ID: 8)  - 春日部つむぎ (ノーマル)");
        println!("    hau                         (ID: 10) - 雨晴はう (ノーマル)");
        println!("    ritsu                       (ID: 9)  - 波音リツ (ノーマル)");
        println!("    takehiro                    (ID: 11) - 玄野武宏 (ノーマル)");
        println!("    kohtaro                     (ID: 12) - 白上虎太郎 (ふつう)");
        println!("    ryusei                      (ID: 13) - 青山龍星 (ノーマル)");
        println!("    sora                        (ID: 16) - 九州そら (ノーマル)");
        println!();

        println!("Usage: voicevox-say --voice <voice_name> \"your text\"");
        println!("Example: voicevox-say --voice zundamon \"こんにちは\"");
        println!();
        println!("💡 Tip: Use --load-all-models to preload all voice models for faster synthesis.");
        println!("💡 Tip: Default models (zundamon, metan, tsumugi) are loaded automatically.");

        std::process::exit(0);
    }

    // 直接一致するボイス名を探す
    if let Some(&(style_id, description)) = voices.get(voice_name) {
        return Ok((style_id, description.to_string()));
    }

    // 数値として解析を試みる
    if let Ok(style_id) = voice_name.parse::<u32>() {
        return Ok((style_id, format!("Style ID {}", style_id)));
    }

    Err(anyhow!(
        "Unknown voice: {}. Use --voice ? to list available voices.",
        voice_name
    ))
}

fn resolve_voice_name(voice_name: &str) -> Result<(u32, String)> {
    let voices = get_voice_mapping();

    // 音声一覧表示の特別なケース
    if voice_name == "?" {
        println!("🎭 Available VOICEVOX voices:");
        println!();

        // キャラクター別にグループ化して表示
        println!("  📝 ずんだもん:");
        println!("    zundamon, zundamon-normal    (ID: 3)  - ずんだもん (ノーマル)");
        println!("    zundamon-amama              (ID: 1)  - ずんだもん (あまあま)");
        println!("    zundamon-tsundere           (ID: 7)  - ずんだもん (ツンツン)");
        println!("    zundamon-sexy               (ID: 5)  - ずんだもん (セクシー)");
        println!("    zundamon-whisper            (ID: 22) - ずんだもん (ささやき)");
        println!("    zundamon-excited            (ID: 38) - ずんだもん (ヘロヘロ)");
        println!();

        println!("  🍊 四国めたん:");
        println!("    metan, metan-normal         (ID: 2)  - 四国めたん (ノーマル)");
        println!("    metan-amama                 (ID: 0)  - 四国めたん (あまあま)");
        println!("    metan-tsundere              (ID: 6)  - 四国めたん (ツンツン)");
        println!("    metan-sexy                  (ID: 4)  - 四国めたん (セクシー)");
        println!("    metan-whisper               (ID: 36) - 四国めたん (ささやき)");
        println!("    metan-excited               (ID: 37) - 四国めたん (ヘロヘロ)");
        println!();

        println!("  🌸 その他のキャラクター:");
        println!("    tsumugi                     (ID: 8)  - 春日部つむぎ (ノーマル)");
        println!("    hau                         (ID: 10) - 雨晴はう (ノーマル)");
        println!("    ritsu                       (ID: 9)  - 波音リツ (ノーマル)");
        println!("    takehiro                    (ID: 11) - 玄野武宏 (ノーマル)");
        println!("    kohtaro                     (ID: 12) - 白上虎太郎 (ふつう)");
        println!("    ryusei                      (ID: 13) - 青山龍星 (ノーマル)");
        println!("    sora                        (ID: 16) - 九州そら (ノーマル)");
        println!();

        println!("  💡 Tips:");
        println!("    - 数値IDを直接指定することも可能です: -v 3");
        println!("    - キャラクター名のみでデフォルトモードを使用: -v zundamon");
        println!("    - 特定のモードを指定: -v zundamon-amama");
        println!();

        std::process::exit(0);
    }

    // 直接的な数値指定をサポート
    if let Ok(style_id) = voice_name.parse::<u32>() {
        return Ok((style_id, format!("Style ID {}", style_id)));
    }

    // 音声名から検索
    if let Some((style_id, description)) = voices.get(voice_name) {
        Ok((*style_id, description.to_string()))
    } else {
        Err(anyhow!(
            "Unknown voice: '{}'. Use -v ? to list available voices.",
            voice_name
        ))
    }
}

// テキスト入力を取得する関数
fn get_input_text(matches: &clap::ArgMatches) -> Result<String> {
    // コマンドライン引数から
    if let Some(text) = matches.get_one::<String>("text") {
        return Ok(text.clone());
    }

    // ファイルから
    if let Some(file_path) = matches.get_one::<String>("input-file") {
        if file_path == "-" {
            // 標準入力から読み取り
            use std::io::{self, Read};
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            return Ok(buffer.trim().to_string());
        } else {
            // ファイルから読み取り
            return Ok(fs::read_to_string(file_path)?);
        }
    }

    // テキストが何も指定されていない場合は標準入力から
    use std::io::{self, Read};
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

// ヘルパー関数：テキストを文に分割
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current_sentence = String::new();

    for ch in text.chars() {
        current_sentence.push(ch);

        // 文の終端文字を検出
        if ch == '。' || ch == '！' || ch == '？' || ch == '.' || ch == '!' || ch == '?' {
            if !current_sentence.trim().is_empty() {
                sentences.push(current_sentence.trim().to_string());
                current_sentence.clear();
            }
        }
    }

    // 残りのテキストがあれば追加
    if !current_sentence.trim().is_empty() {
        sentences.push(current_sentence.trim().to_string());
    }

    // 空の文を除外
    sentences
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect()
}

// ヘルパー関数：テキストを指定した文字数で分割
fn split_text_by_size(text: &str, max_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    for word in text.split_whitespace() {
        if current_chunk.len() + word.len() + 1 > max_size && !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
            current_chunk.clear();
        }

        if !current_chunk.is_empty() {
            current_chunk.push(' ');
        }
        current_chunk.push_str(word);
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    chunks
}

fn main() -> Result<()> {
    let app = Command::new("voicevox-say")
        .version(env!("CARGO_PKG_VERSION"))
        .about("🫛 VOICEVOX Say - Convert text to audible speech using VOICEVOX")
        .arg(
            Arg::new("text")
                .help("Specify the text to speak on the command line")
                .index(1)
                .required(false),
        )
        .arg(
            Arg::new("voice")
                .help("Specify the voice to be used. Use '?' to list all available voices")
                .long("voice")
                .short('v')
                .value_name("VOICE")
                .default_value("zundamon"),
        )
        .arg(
            Arg::new("rate")
                .help("Speech rate multiplier (0.5-2.0, default: 1.0)")
                .long("rate")
                .short('r')
                .value_name("RATE")
                .value_parser(clap::value_parser!(f32))
                .default_value("1.0"),
        )
        .arg(
            Arg::new("output-file")
                .help("Specify the path for an audio file to be written")
                .long("output-file")
                .short('o')
                .value_name("FILE"),
        )
        .arg(
            Arg::new("input-file")
                .help("Specify a file to be spoken. Use '-' for stdin")
                .long("input-file")
                .short('f')
                .value_name("FILE"),
        )
        .arg(
            Arg::new("streaming")
                .help("Enable streaming synthesis (sentence-by-sentence)")
                .long("streaming")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("quiet")
                .help("Don't play audio, only save to file")
                .long("quiet")
                .short('q')
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list-speakers")
                .help("List all available speakers and styles from loaded models")
                .long("list-speakers")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("speaker-id")
                .help("Directly specify speaker style ID (advanced users)")
                .long("speaker-id")
                .short('s')
                .value_name("ID")
                .value_parser(clap::value_parser!(u32))
                .conflicts_with("voice"),
        )
        .arg(
            Arg::new("load-all-models")
                .help("Load all available VVM models (slower startup, all voices available)")
                .long("load-all-models")
                .action(clap::ArgAction::SetTrue),
        );

    let matches = app.get_matches();

    // 音声一覧表示の処理（早期リターン）
    if let Some(voice_name) = matches.get_one::<String>("voice") {
        if voice_name == "?" {
            resolve_voice_name("?")?; // これは内部でexit(0)する
        }
    }

    // Initialize VOICEVOX Core
    println!("🚀 Initializing VOICEVOX Core...");
    let mut core = VoicevoxCore::new()?;

    // Load all models if requested
    if matches.get_flag("load-all-models") {
        println!("📦 Loading all VVM models (--load-all-models specified)...");
        if let Err(e) = VoicevoxCore::load_models(core.synthesizer) {
            println!("⚠️  Warning: Failed to load some models: {}", e);
        }
    }

    println!("✅ VOICEVOX Core initialized successfully");

    // 詳細なスピーカー一覧表示
    if matches.get_flag("list-speakers") {
        println!("📋 All available speakers and styles from loaded models:");
        let speakers = core.get_speakers()?;
        for speaker in &speakers {
            println!("  👤 {}", speaker.name);
            for style in &speaker.styles {
                println!("    🎭 {} (ID: {})", style.name, style.id);
                if let Some(style_type) = &style.style_type {
                    println!("        Type: {}", style_type);
                }
            }
            println!();
        }
        return Ok(());
    }

    // テキスト入力を取得
    let text = get_input_text(&matches)?;
    if text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }

    // 音声設定を解決（speaker-idが指定されている場合はそちらを優先）
    let (style_id, voice_description) =
        if let Some(speaker_id) = matches.get_one::<u32>("speaker-id") {
            (*speaker_id, format!("Style ID {}", speaker_id))
        } else {
            let voice_name = matches.get_one::<String>("voice").unwrap();
            resolve_voice_name(voice_name)?
        };

    // 設定パラメータ
    let use_streaming = matches.get_flag("streaming");
    let rate = *matches.get_one::<f32>("rate").unwrap_or(&1.0);

    // レート範囲チェック
    if rate < 0.5 || rate > 2.0 {
        return Err(anyhow!("Rate must be between 0.5 and 2.0, got: {}", rate));
    }

    println!("🎭 Voice: {}", voice_description);
    if rate != 1.0 {
        println!("⚡ Rate: {}x", rate);
    }

    // 必要なモデルを動的に読み込み（合成直前に実行）
    if !matches.get_flag("load-all-models") {
        if let Some(model_num) = get_model_for_voice_id(style_id) {
            println!(
                "📦 Loading required model for style ID {}: {}.vvm",
                style_id, model_num
            );
            if let Err(e) = core.load_specific_model(&model_num.to_string()) {
                return Err(anyhow!(
                    "Failed to load required model for style ID {}: {}",
                    style_id,
                    e
                ));
            }
        } else {
            return Err(anyhow!("Unknown voice model required for style ID {}. Please ensure the voice model is available.", style_id));
        }
    }

    // 音声合成の実行
    if use_streaming {
        println!("🎵 Starting streaming synthesis...");
        core.synthesize_streaming_with_config(&text, style_id, 100, None)?;
    } else {
        println!("🎤 Synthesizing speech...");
        let wav_data = core.synthesize_simple(&text, style_id)?;
        println!("✅ Speech synthesis completed ({} bytes)", wav_data.len());

        // ファイル出力
        if let Some(output_file) = matches.get_one::<String>("output-file") {
            fs::write(output_file, &wav_data)?;
            println!("💾 Audio saved to: {}", output_file);
        }

        // 音声再生（quietモードでない場合）
        if !matches.get_flag("quiet") && matches.get_one::<String>("output-file").is_none() {
            let temp_file = "/tmp/voicevox_say_temp.wav";
            fs::write(temp_file, &wav_data)?;

            // macOS標準のafplayで再生
            if let Ok(_) = std::process::Command::new("afplay").arg(temp_file).output() {
                // 成功時は何も表示しない（sayコマンドと同様）
            } else if let Ok(_) = std::process::Command::new("play").arg(temp_file).output() {
                // soxでの再生もサイレント
            } else {
                eprintln!("Warning: No audio player found. Install sox or use -o to save file");
            }

            // 一時ファイルの削除
            let _ = fs::remove_file(temp_file);
        }
    }

    Ok(())
}
