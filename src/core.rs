use anyhow::{anyhow, Result};
use std::path::PathBuf;
use voicevox_core::{
    blocking::{Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile},
    AccelerationMode,
};

use crate::paths::{find_models_dir, find_models_dir_client, find_openjtalk_dict};
use crate::voice::Speaker;

/// VOICEVOX Core wrapper using official Rust implementation with static linking
///
/// This struct provides a high-level interface to VOICEVOX Core 0.16.0, managing
/// the synthesizer, OpenJTalk text analyzer, and ONNX Runtime components with
/// functional programming patterns.
///
/// # Architecture
///
/// - **Static Linking**: Uses statically linked VOICEVOX Core libraries for zero dependencies
/// - **Functional Design**: Immutable operations with monadic error handling
/// - **CPU-Only**: Optimized for Apple Silicon with CPU-only processing
/// - **Thread-Safe**: Automatically detects optimal CPU thread count
///
/// # Example
///
/// ```rust,no_run
/// use voicevox_cli::VoicevoxCore;
/// use std::path::PathBuf;
///
/// // Initialize with static linking (no runtime dependencies)
/// let mut core = VoicevoxCore::new()?;
///
/// // Load a voice model dynamically
/// let model_path = PathBuf::from("path/to/model.vvm");
/// core.load_model(&model_path)?;
///
/// // Synthesize speech with speaker ID
/// let audio_data = core.synthesize("こんにちは、ずんだもんなのだ", 3)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct VoicevoxCore {
    synthesizer: Synthesizer<OpenJtalk>,
}

impl VoicevoxCore {
    /// Creates a new VoicevoxCore instance with static linking
    ///
    /// Initializes ONNX Runtime and OpenJTalk with embedded dictionary.
    /// Uses CPU-only acceleration mode optimized for Apple Silicon.
    ///
    /// # Returns
    ///
    /// A configured VoicevoxCore instance ready for model loading
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - ONNX Runtime initialization fails
    /// - OpenJTalk dictionary not found or invalid
    /// - Synthesizer configuration is invalid
    pub fn new() -> Result<Self> {
        let onnxruntime = Onnxruntime::init_once()
            .map_err(|e| anyhow!("Failed to initialize ONNX Runtime: {}", e))?;
        
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

    /// Loads all available voice models from the models directory
    ///
    /// Recursively scans the models directory and loads all VVM files found.
    /// May trigger first-run setup if models are not found.
    ///
    /// # Returns
    ///
    /// Success if at least one model was loaded
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Models directory not found and setup fails
    /// - No valid VVM files found in directory
    /// - All model loading attempts fail
    pub fn load_all_models(&self) -> Result<()> {
        // Find the models directory - this may trigger first-run setup
        let models_dir = find_models_dir()?;
        self.load_vvm_files_recursive(&models_dir)
    }

    /// Loads all available voice models without attempting download
    ///
    /// Client-side model loading that skips first-run setup and download attempts.
    /// Used by daemon client to avoid triggering downloads on the server side.
    ///
    /// # Returns
    ///
    /// Success if at least one model was loaded
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Models directory not found (no download attempt made)
    /// - No valid VVM files found in directory
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

    /// Synthesizes speech from text using the specified voice style
    ///
    /// Converts Japanese text to speech using the loaded voice models and the specified style ID.
    /// The text is processed through OpenJTalk for phonetic analysis and then synthesized
    /// using VOICEVOX Core neural networks.
    ///
    /// # Arguments
    ///
    /// * `text` - Japanese text to synthesize (UTF-8)
    /// * `style_id` - Voice style identifier (speaker + emotional style)
    ///
    /// # Returns
    ///
    /// WAV audio data as bytes on success
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Text analysis fails (invalid Japanese text)
    /// - Style ID not found in loaded models
    /// - Neural network synthesis fails
    /// - Memory allocation fails during processing
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use voicevox_cli::VoicevoxCore;
    /// # let core = VoicevoxCore::new()?;
    /// // Synthesize with Zundamon's normal voice (style ID 3)
    /// let audio = core.synthesize("こんにちは、ずんだもんなのだ", 3)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
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