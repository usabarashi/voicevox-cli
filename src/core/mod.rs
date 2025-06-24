use anyhow::{anyhow, Result};
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::ptr;

use crate::bindings::*;
use crate::paths::{find_models_dir, find_models_dir_client, find_openjtalk_dict};
use crate::voice::Speaker;

// VOICEVOX Core wrapper
pub struct VoicevoxCore {
    synthesizer: *mut VoicevoxSynthesizer,
    _open_jtalk_rc: *mut OpenJtalkRc,
    #[cfg(feature = "dynamic_voicevox")]
    _dynamic_core: Option<DynamicVoicevoxCore>,
}

unsafe impl Send for VoicevoxCore {}
unsafe impl Sync for VoicevoxCore {}

impl VoicevoxCore {
    pub fn new() -> Result<Self> {
        #[cfg(feature = "dynamic_voicevox")]
        {
            let dynamic_core = DynamicVoicevoxCore::new()?;
            Self::new_with_dynamic_core(dynamic_core)
        }
        #[cfg(not(feature = "dynamic_voicevox"))]
        {
            Self::new_with_linked()
        }
    }

    #[cfg(feature = "dynamic_voicevox")]
    fn new_with_dynamic_core(dynamic_core: DynamicVoicevoxCore) -> Result<Self> {
        unsafe {
            // Load ONNX Runtime first
            let load_options = (dynamic_core.voicevox_make_default_load_onnxruntime_options)();
            let mut onnxruntime: *const VoicevoxOnnxruntime = ptr::null();

            let result =
                (dynamic_core.voicevox_onnxruntime_load_once)(load_options, &mut onnxruntime);
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

            let result =
                (dynamic_core.voicevox_open_jtalk_rc_new)(dict_cstr.as_ptr(), &mut open_jtalk_rc);
            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!(
                    "OpenJTalk RC initialization failed: code {}",
                    result
                ));
            }

            // Create synthesizer with CPU-only mode for macOS
            let init_options = VoicevoxInitializeOptions {
                acceleration_mode: VoicevoxAccelerationMode::Cpu, // Force CPU mode, no GPU testing
                cpu_num_threads: 0, // Use default number of CPU threads (0 = auto-detect)
            };

            let mut synthesizer: *mut VoicevoxSynthesizer = ptr::null_mut();
            let result = (dynamic_core.voicevox_synthesizer_new)(
                onnxruntime,
                open_jtalk_rc,
                init_options,
                &mut synthesizer,
            );

            if result != VOICEVOX_RESULT_OK {
                (dynamic_core.voicevox_open_jtalk_rc_delete)(open_jtalk_rc);
                return Err(anyhow!("Synthesizer creation failed: code {}", result));
            }

            Ok(VoicevoxCore {
                synthesizer,
                _open_jtalk_rc: open_jtalk_rc,
                _dynamic_core: Some(dynamic_core),
            })
        }
    }

    #[cfg(feature = "link_voicevox")]
    fn new_with_linked() -> Result<Self> {
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

            Ok(VoicevoxCore {
                synthesizer,
                _open_jtalk_rc: open_jtalk_rc,
                #[cfg(feature = "dynamic_voicevox")]
                _dynamic_core: None,
            })
        }
    }

    pub fn load_all_models(&self) -> Result<()> {
        // Find the models directory - this may trigger first-run setup
        let models_dir = find_models_dir()?;

        // Load all VVM files recursively
        let mut loaded_count = 0;
        self.load_vvm_files_recursive(&models_dir, &mut loaded_count)?;

        if loaded_count == 0 {
            return Err(anyhow!("Failed to load any models"));
        }

        Ok(())
    }

    // Client-side model loading (no download attempt)
    pub fn load_all_models_no_download(&self) -> Result<()> {
        // Find the models directory - no download attempt for client side
        let models_dir = find_models_dir_client()?;

        // Load all VVM files recursively
        let mut loaded_count = 0;
        self.load_vvm_files_recursive(&models_dir, &mut loaded_count)?;

        if loaded_count == 0 {
            return Err(anyhow!("Failed to load any models"));
        }

        Ok(())
    }
    
    // Helper function to recursively load VVM files from a directory
    fn load_vvm_files_recursive(&self, dir: &PathBuf, loaded_count: &mut i32) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let entry_path = entry.path();
                
                if entry_path.is_file() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.ends_with(".vvm") {
                            if let Some(path_str) = entry_path.to_str() {
                                if let Ok(path_cstr) = CString::new(path_str) {
                                    unsafe {
                                        let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                                        let result = voicevox_voice_model_file_open(
                                            path_cstr.as_ptr(),
                                            &mut model,
                                        );
                                        if result == VOICEVOX_RESULT_OK {
                                            let load_result = voicevox_synthesizer_load_voice_model(
                                                self.synthesizer,
                                                model,
                                            );
                                            if load_result == VOICEVOX_RESULT_OK {
                                                *loaded_count += 1;
                                            } else if load_result == 18 {
                                                // MODEL_ALREADY_LOADED_ERROR - not an error
                                                *loaded_count += 1;
                                            }
                                            voicevox_voice_model_file_delete(model);
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if entry_path.is_dir() {
                    // Recursively search subdirectories
                    self.load_vvm_files_recursive(&entry_path, loaded_count)?;
                }
            }
        }
        Ok(())
    }

    pub fn load_minimal_models(&self) -> Result<()> {
        // Load only essential models for faster startup (minimal mode)
        // Priority: ずんだもん (3.vvm), 四国めたん (2.vvm), 春日部つむぎ (8.vvm)
        let default_models = ["3.vvm", "2.vvm", "8.vvm"];

        let models_dir = find_models_dir_client()?;

        // Silent minimal model loading with recursive search
        let mut loaded_count = 0;
        for model_name in &default_models {
            if let Some(model_path) = self.find_model_file_recursive(&models_dir, model_name) {
                if let Some(path_str) = model_path.to_str() {
                    if let Ok(path_cstr) = CString::new(path_str) {
                        unsafe {
                            let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                            let result =
                                voicevox_voice_model_file_open(path_cstr.as_ptr(), &mut model);
                            if result == VOICEVOX_RESULT_OK {
                                let load_result =
                                    voicevox_synthesizer_load_voice_model(self.synthesizer, model);
                                if load_result == VOICEVOX_RESULT_OK {
                                    loaded_count += 1;
                                } else if load_result == 18 {
                                    // MODEL_ALREADY_LOADED_ERROR
                                    loaded_count += 1;
                                }
                                voicevox_voice_model_file_delete(model);
                            }
                        }
                    }
                }
            }
        }

        if loaded_count == 0 {
            return Err(anyhow!("No minimal VVM models were loaded"));
        }

        Ok(())
    }
    
    // Helper function to find a specific model file recursively
    fn find_model_file_recursive(&self, dir: &PathBuf, target_filename: &str) -> Option<PathBuf> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let entry_path = entry.path();
                
                if entry_path.is_file() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name == target_filename {
                            return Some(entry_path);
                        }
                    }
                } else if entry_path.is_dir() {
                    // Recursively search subdirectories
                    if let Some(found) = self.find_model_file_recursive(&entry_path, target_filename) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    pub fn load_specific_model(&self, model_name: &str) -> Result<()> {
        let models_dir = find_models_dir_client()?;
        let model_path = models_dir.join(format!("{}.vvm", model_name));

        if !model_path.exists() {
            return Err(anyhow!("Model not found: {}.vvm", model_name));
        }

        println!("Loading model: {}.vvm", model_name);

        if let Some(path_str) = model_path.to_str() {
            if let Ok(path_cstr) = CString::new(path_str) {
                unsafe {
                    let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                    let result = voicevox_voice_model_file_open(path_cstr.as_ptr(), &mut model);
                    if result == VOICEVOX_RESULT_OK {
                        let load_result =
                            voicevox_synthesizer_load_voice_model(self.synthesizer, model);
                        if load_result == VOICEVOX_RESULT_OK {
                            println!("  Successfully loaded: {}.vvm", model_name);
                        } else if load_result == 18 {
                            // MODEL_ALREADY_LOADED_ERROR
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

    pub fn synthesize(&self, text: &str, style_id: u32) -> Result<Vec<u8>> {
        self.synthesize_real(text, style_id)
    }

    fn synthesize_real(&self, text: &str, style_id: u32) -> Result<Vec<u8>> {
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

    pub fn get_speakers(&self) -> Result<Vec<Speaker>> {
        self.get_speakers_real()
    }

    fn get_speakers_real(&self) -> Result<Vec<Speaker>> {
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