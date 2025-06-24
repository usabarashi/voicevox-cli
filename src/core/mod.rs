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
        
        let loaded_count = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .map(|path| self.process_entry_path(&path))
            .sum::<u32>();

        (loaded_count > 0)
            .then_some(())
            .ok_or_else(|| anyhow!("Failed to load any models"))
    }
    
    fn process_entry_path(&self, path: &PathBuf) -> u32 {
        match path {
            p if p.is_file() => self.try_load_vvm_file(p),
            p if p.is_dir() => self.count_loaded_models_in_dir(p),
            _ => 0,
        }
    }
    
    fn count_loaded_models_in_dir(&self, dir: &PathBuf) -> u32 {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| self.process_entry_path(&entry.path()))
                    .sum()
            })
            .unwrap_or(0)
    }
    
    fn try_load_vvm_file(&self, file_path: &PathBuf) -> u32 {
        file_path
            .file_name()
            .and_then(|f| f.to_str())
            .filter(|name| name.ends_with(".vvm"))
            .and_then(|_| VoiceModelFile::open(file_path).ok())
            .and_then(|model| self.synthesizer.load_voice_model(&model).ok())
            .map(|_| 1)
            .unwrap_or(0)
    }


    pub fn load_specific_model(&self, model_name: &str) -> Result<()> {
        let models_dir = find_models_dir_client()?;
        let model_path = models_dir.join(format!("{}.vvm", model_name));

        model_path
            .exists()
            .then_some(())
            .ok_or_else(|| anyhow!("Model not found: {}.vvm", model_name))
            .and_then(|_| {
                VoiceModelFile::open(&model_path)
                    .map_err(|e| anyhow!("Failed to open model {}: {}", model_name, e))
            })
            .and_then(|model| {
                self.synthesizer
                    .load_voice_model(&model)
                    .map_err(|e| anyhow!("Failed to load model {}: {}", model_name, e))
            })
    }

    pub fn synthesize(&self, text: &str, style_id: u32) -> Result<Vec<u8>> {
        use voicevox_core::StyleId;
        
        self.synthesizer
            .tts(text, StyleId::new(style_id))
            .perform()
            .map_err(|e| anyhow!("Speech synthesis failed: {}", e))
    }

    pub fn get_speakers(&self) -> Result<Vec<Speaker>> {
        // Convert VOICEVOX Core metadata to our Speaker format using functional composition
        let speakers = self
            .synthesizer
            .metas()
            .iter()
            .map(|meta| Speaker {
                name: meta.name.clone(),
                speaker_uuid: meta.speaker_uuid.clone(),
                styles: meta
                    .styles
                    .iter()
                    .map(|style| crate::voice::Style {
                        name: style.name.clone(),
                        id: style.id.0,
                        style_type: Some(format!("{:?}", style.r#type)),
                    })
                    .collect(),
                version: meta.version.to_string(),
            })
            .collect();

        Ok(speakers)
    }
}