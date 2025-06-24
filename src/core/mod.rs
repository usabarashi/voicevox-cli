use anyhow::{anyhow, Result};
use std::path::PathBuf;
use voicevox_core::{
    blocking::{Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile},
    AccelerationMode,
};

use crate::paths::{find_models_dir, find_models_dir_client, find_openjtalk_dict};
use crate::voice::Speaker;

// VOICEVOX Core wrapper using official Rust implementation
pub struct VoicevoxCore {
    synthesizer: Synthesizer<OpenJtalk>,
}

impl VoicevoxCore {
    pub fn new() -> Result<Self> {
        let onnxruntime = Onnxruntime::load_once().perform()
            .map_err(|e| anyhow!("Failed to load ONNX Runtime: {}", e))?;
        
        let dict_path = find_openjtalk_dict()?;
        
        let open_jtalk = OpenJtalk::new(dict_path)
            .map_err(|e| anyhow!("Failed to initialize OpenJTalk: {}", e))?;
        
        let synthesizer = Synthesizer::builder(&onnxruntime)
            .text_analyzer(open_jtalk)
            .acceleration_mode(AccelerationMode::Cpu)
            .cpu_num_threads(0) // Auto-detect CPU threads
            .build()
            .map_err(|e| anyhow!("Failed to create synthesizer: {}", e))?;

        Ok(VoicevoxCore { synthesizer })
    }

    pub fn load_all_models(&self) -> Result<()> {
        // Find the models directory - this may trigger first-run setup
        let models_dir = find_models_dir()?;
        self.load_vvm_files_recursive(&models_dir)
    }

    // Client-side model loading (no download attempt)
    pub fn load_all_models_no_download(&self) -> Result<()> {
        // Find the models directory - no download attempt for client side
        let models_dir = find_models_dir_client()?;
        self.load_vvm_files_recursive(&models_dir)
    }
    
    fn load_vvm_files_recursive(&self, dir: &PathBuf) -> Result<()> {
        let entries = std::fs::read_dir(dir)?;
        let mut loaded_count = 0;
        
        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            
            if entry_path.is_file() {
                loaded_count += self.try_load_vvm_file(&entry_path);
            } else if entry_path.is_dir() {
                let _ = self.load_vvm_files_recursive(&entry_path);
            }
        }

        if loaded_count == 0 {
            return Err(anyhow!("Failed to load any models"));
        }

        Ok(())
    }
    
    fn try_load_vvm_file(&self, file_path: &PathBuf) -> u32 {
        let _file_name = match file_path.file_name().and_then(|f| f.to_str()) {
            Some(name) if name.ends_with(".vvm") => name,
            _ => return 0,
        };
        
        let model = match VoiceModelFile::open(file_path) {
            Ok(model) => model,
            Err(_) => return 0,
        };
        
        match self.synthesizer.load_voice_model(&model) {
            Ok(_) => 1,
            Err(_) => 0, // Model already loaded or other non-critical error
        }
    }

    pub fn load_minimal_models(&self) -> Result<()> {
        // Load only essential models for faster startup (minimal mode)
        // Dynamically discover and load first few available models
        let models_dir = find_models_dir_client()?;
        let vvm_files = self.find_vvm_files_limited(&models_dir, 3)?; // Limit to first 3 models

        let mut loaded_count = 0;
        for model_path in vvm_files {
            match VoiceModelFile::open(&model_path) {
                Ok(model) => {
                    match self.synthesizer.load_voice_model(&model) {
                        Ok(_) => loaded_count += 1,
                        Err(_) => {
                            // Model already loaded or other non-critical error
                            loaded_count += 1; // Count as success for minimal loading
                        }
                    }
                }
                Err(_) => {
                    // Failed to open model file, continue with others
                }
            }
        }

        if loaded_count == 0 {
            return Err(anyhow!("No minimal VVM models were loaded"));
        }

        Ok(())
    }
    
    // Helper function to find limited number of VVM files for minimal loading
    fn find_vvm_files_limited(&self, dir: &PathBuf, limit: usize) -> Result<Vec<PathBuf>> {
        let mut vvm_files = Vec::new();
        
        if !dir.exists() {
            return Ok(vvm_files);
        }
        
        let entries = std::fs::read_dir(dir)?;
        
        for entry in entries.filter_map(|e| e.ok()) {
            if vvm_files.len() >= limit {
                break;
            }
            
            let path = entry.path();
            
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "vvm" {
                        vvm_files.push(path);
                    }
                }
            } else if path.is_dir() {
                let mut sub_files = self.find_vvm_files_limited(&path, limit - vvm_files.len())?;
                vvm_files.append(&mut sub_files);
            }
        }
        
        Ok(vvm_files)
    }
    

    pub fn load_specific_model(&self, model_name: &str) -> Result<()> {
        let models_dir = find_models_dir_client()?;
        let model_path = models_dir.join(format!("{}.vvm", model_name));

        if !model_path.exists() {
            return Err(anyhow!("Model not found: {}.vvm", model_name));
        }

        let model = VoiceModelFile::open(&model_path)
            .map_err(|e| anyhow!("Failed to open model {}: {}", model_name, e))?;
        
        self.synthesizer.load_voice_model(&model)
            .map_err(|e| anyhow!("Failed to load model {}: {}", model_name, e))?;

        Ok(())
    }

    pub fn synthesize(&self, text: &str, style_id: u32) -> Result<Vec<u8>> {
        use voicevox_core::StyleId;
        
        self.synthesizer
            .tts(text, StyleId::new(style_id))
            .perform()
            .map_err(|e| anyhow!("Speech synthesis failed: {}", e))
    }

    pub fn get_speakers(&self) -> Result<Vec<Speaker>> {
        let metas = self.synthesizer.metas();
        
        // Convert VOICEVOX Core metadata to our Speaker format
        let speakers: Vec<Speaker> = metas
            .iter()
            .map(|meta| Speaker {
                name: meta.name.clone(),
                speaker_uuid: meta.speaker_uuid.clone(),
                styles: meta.styles.iter().map(|style| crate::voice::Style {
                    name: style.name.clone(),
                    id: style.id.0,
                    style_type: Some(format!("{:?}", style.r#type)),
                }).collect(),
                version: meta.version.to_string(),
            })
            .collect();

        Ok(speakers)
    }
}