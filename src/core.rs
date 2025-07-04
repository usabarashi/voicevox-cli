use anyhow::{anyhow, Result};
use std::path::PathBuf;
use voicevox_core::{
    blocking::{Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile},
    AccelerationMode,
};

use crate::paths::{find_models_dir, find_models_dir_client, find_openjtalk_dict};
use crate::voice::Speaker;

pub trait CoreSynthesis {
    type Error;
    type Output<'a>: AsRef<[u8]>
    where
        Self: 'a;
    type SpeakerData<'a>: AsRef<[Speaker]>
    where
        Self: 'a;

    fn synthesize<'a>(&'a self, text: &str, style_id: u32)
        -> Result<Self::Output<'a>, Self::Error>;
    fn get_speakers(&self) -> Result<Self::SpeakerData<'_>, Self::Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreConfig<const CPU_THREADS: usize = 0, const BUFFER_SIZE: usize = 8192>;

impl<const CPU_THREADS: usize, const BUFFER_SIZE: usize> Default
    for CoreConfig<CPU_THREADS, BUFFER_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const CPU_THREADS: usize, const BUFFER_SIZE: usize> CoreConfig<CPU_THREADS, BUFFER_SIZE> {
    pub const fn new() -> Self {
        Self
    }

    pub const fn cpu_threads() -> usize {
        if CPU_THREADS == 0 {
            0 // Auto-detect at runtime
        } else {
            CPU_THREADS
        }
    }

    pub const fn buffer_size() -> usize {
        BUFFER_SIZE
    }
}

/// VOICEVOX Core wrapper with static linking to libvoicevox_core.dylib
pub struct VoicevoxCore<Config = CoreConfig<0, 8192>> {
    synthesizer: Synthesizer<OpenJtalk>,
    #[allow(dead_code)]
    config: Config,
}

impl<Config> VoicevoxCore<Config>
where
    Config: Clone + Send + Sync,
{
    /// Creates a new VoicevoxCore instance with CPU-only acceleration
    pub fn with_config(config: Config) -> Result<Self> {
        let onnxruntime = Onnxruntime::init_once()
            .map_err(|e| anyhow!("Failed to initialize ONNX Runtime: {}", e))?;

        let dict_path = find_openjtalk_dict()?;

        let open_jtalk = OpenJtalk::new(dict_path)
            .map_err(|e| anyhow!("Failed to initialize OpenJTalk: {}", e))?;

        let synthesizer = Synthesizer::builder(onnxruntime)
            .text_analyzer(open_jtalk)
            .acceleration_mode(AccelerationMode::Cpu)
            .cpu_num_threads(0) // Auto-detect CPU threads
            .build()
            .map_err(|e| anyhow!("Failed to create synthesizer: {}", e))?;

        Ok(VoicevoxCore {
            synthesizer,
            config,
        })
    }
}

impl VoicevoxCore<CoreConfig<0, 8192>> {
    pub fn new() -> Result<Self> {
        Self::with_config(CoreConfig::new())
    }
}

impl<Config> CoreSynthesis for VoicevoxCore<Config>
where
    Config: Clone + Send + Sync,
{
    type Error = anyhow::Error;
    type Output<'a>
        = Vec<u8>
    where
        Self: 'a;
    type SpeakerData<'a>
        = Vec<Speaker>
    where
        Self: 'a;

    fn synthesize<'a>(
        &'a self,
        text: &str,
        style_id: u32,
    ) -> Result<Self::Output<'a>, Self::Error> {
        use voicevox_core::StyleId;

        self.synthesizer
            .tts(text, StyleId::new(style_id))
            .perform()
            .map_err(|e| anyhow!("Speech synthesis failed: {}", e))
    }

    fn get_speakers(&self) -> Result<Self::SpeakerData<'_>, Self::Error> {
        let speakers = self
            .synthesizer
            .metas()
            .iter()
            .map(|meta| Speaker {
                #[cfg(feature = "compact_str")]
                name: meta.name.clone().into(),
                #[cfg(not(feature = "compact_str"))]
                name: meta.name.clone(),
                #[cfg(feature = "compact_str")]
                speaker_uuid: meta.speaker_uuid.clone().into(),
                #[cfg(not(feature = "compact_str"))]
                speaker_uuid: meta.speaker_uuid.clone(),
                styles: meta
                    .styles
                    .iter()
                    .map(|style| crate::voice::Style {
                        #[cfg(feature = "compact_str")]
                        name: style.name.clone().into(),
                        #[cfg(not(feature = "compact_str"))]
                        name: style.name.clone(),
                        id: style.id.0,
                        #[cfg(feature = "compact_str")]
                        style_type: Some(format!("{:?}", style.r#type).into()),
                        #[cfg(not(feature = "compact_str"))]
                        style_type: Some(format!("{:?}", style.r#type)),
                    })
                    .collect(),
                #[cfg(feature = "compact_str")]
                version: meta.version.to_string().into(),
                #[cfg(not(feature = "compact_str"))]
                version: meta.version.to_string(),
            })
            .collect();

        Ok(speakers)
    }
}

impl<Config> VoicevoxCore<Config>
where
    Config: Clone + Send + Sync,
{
    /// Load all available VVM models, may trigger first-run setup
    pub fn load_all_models(&self) -> Result<()> {
        let models_dir = find_models_dir()?;
        self.load_vvm_files_recursive(&models_dir)
    }

    /// Load all available VVM models without triggering downloads
    pub fn load_all_models_no_download(&self) -> Result<()> {
        let models_dir = find_models_dir_client()?;
        self.load_vvm_files_recursive(&models_dir)
    }

    fn load_vvm_files_recursive(&self, dir: &PathBuf) -> Result<()> {
        let entries = std::fs::read_dir(dir)?;

        let loaded_count = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter_map(|path| self.process_entry_path(&path).ok())
            .sum::<u32>();

        (loaded_count > 0)
            .then_some(())
            .ok_or_else(|| anyhow!("Failed to load any models"))
    }

    fn process_entry_path(&self, path: &PathBuf) -> Result<u32> {
        match path {
            p if p.is_file() => self.try_load_vvm_file(p),
            p if p.is_dir() => self.count_loaded_models_in_dir(p),
            _ => Ok(0),
        }
    }

    fn count_loaded_models_in_dir(&self, dir: &PathBuf) -> Result<u32> {
        std::fs::read_dir(dir)
            .map_err(|e| anyhow!("Failed to read directory {}: {}", dir.display(), e))?
            .filter_map(Result::ok)
            .map(|entry| self.process_entry_path(&entry.path()))
            .collect::<Result<Vec<_>>>()
            .map(|counts| counts.into_iter().sum())
    }

    fn try_load_vvm_file(&self, file_path: &PathBuf) -> Result<u32> {
        file_path
            .file_name()
            .and_then(|f| f.to_str())
            .filter(|name| name.ends_with(".vvm"))
            .ok_or_else(|| anyhow!("Invalid VVM file path: {}", file_path.display()))?;

        VoiceModelFile::open(file_path)
            .map_err(|e| anyhow!("Failed to open VVM file {}: {}", file_path.display(), e))
            .and_then(|model| {
                self.synthesizer
                    .load_voice_model(&model)
                    .map_err(|e| {
                        anyhow!("Failed to load voice model {}: {}", file_path.display(), e)
                    })
                    .map(|_| 1)
            })
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

    /// Get the voice model ID from loaded model's metadata
    fn get_voice_model_id(&self, model_path: &str) -> Result<voicevox_core::VoiceModelId> {
        let model = VoiceModelFile::open(model_path)
            .map_err(|e| anyhow!("Failed to open model file: {}", e))?;
        
        // Get the voice model ID from the model's metadata
        let model_id = model.id().clone();
        Ok(model_id)
    }

    /// Unload a voice model by its numeric ID
    /// Note: This requires knowing the actual VoiceModelId, which is typically obtained when loading
    pub fn unload_voice_model_by_path(&self, model_path: &str) -> Result<()> {
        let voice_model_id = self.get_voice_model_id(model_path)?;
        
        self.synthesizer
            .unload_voice_model(voice_model_id)
            .map_err(|e| anyhow!("Failed to unload model: {}", e))
    }

    /// Synthesize Japanese text to speech using the specified voice style
    pub fn synthesize(&self, text: &str, style_id: u32) -> Result<Vec<u8>> {
        use voicevox_core::StyleId;

        if text.trim().is_empty() {
            return Err(anyhow!("Empty text provided for synthesis"));
        }

        self.synthesizer
            .tts(text, StyleId::new(style_id))
            .perform()
            .map_err(|e| anyhow!("Speech synthesis failed for style_id {}: {}", style_id, e))
    }

    pub fn get_speakers(&self) -> Result<Vec<Speaker>> {
        let speakers: Vec<Speaker> = self
            .synthesizer
            .metas()
            .iter()
            .map(|meta| {
                let styles = meta
                    .styles
                    .iter()
                    .map(|style| crate::voice::Style {
                        #[cfg(feature = "compact_str")]
                        name: style.name.clone().into(),
                        #[cfg(not(feature = "compact_str"))]
                        name: style.name.clone(),
                        id: style.id.0,
                        #[cfg(feature = "compact_str")]
                        style_type: Some(format!("{:?}", style.r#type).into()),
                        #[cfg(not(feature = "compact_str"))]
                        style_type: Some(format!("{:?}", style.r#type)),
                    })
                    .collect();

                Speaker {
                    #[cfg(feature = "compact_str")]
                    name: meta.name.clone().into(),
                    #[cfg(not(feature = "compact_str"))]
                    name: meta.name.clone(),
                    #[cfg(feature = "compact_str")]
                    speaker_uuid: meta.speaker_uuid.clone().into(),
                    #[cfg(not(feature = "compact_str"))]
                    speaker_uuid: meta.speaker_uuid.clone(),
                    styles,
                    #[cfg(feature = "compact_str")]
                    version: meta.version.to_string().into(),
                    #[cfg(not(feature = "compact_str"))]
                    version: meta.version.to_string(),
                }
            })
            .collect();

        (!speakers.is_empty())
            .then_some(speakers)
            .ok_or_else(|| anyhow!("No speakers found in loaded models"))
    }
}
