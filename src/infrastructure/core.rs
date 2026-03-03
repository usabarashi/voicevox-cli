use anyhow::{anyhow, Result};
use std::path::Path;
use voicevox_core::{
    blocking::{OpenJtalk, Synthesizer},
    AccelerationMode, StyleId,
};

use crate::infrastructure::ipc::{
    is_valid_synthesis_rate, DEFAULT_SYNTHESIS_RATE, MAX_SYNTHESIS_RATE, MIN_SYNTHESIS_RATE,
};
use crate::infrastructure::onnxruntime;
use crate::infrastructure::openjtalk;
use crate::infrastructure::voicevox::{
    open_voice_model_file, open_voice_model_file_by_id, Speaker,
};

pub trait CoreSynthesis {
    type Error;
    type Output<'a>: AsRef<[u8]>
    where
        Self: 'a;
    type SpeakerData<'a>: AsRef<[Speaker]>
    where
        Self: 'a;

    /// Synthesizes audio for the given text and style.
    ///
    /// # Errors
    ///
    /// Returns an implementation-specific error if synthesis fails.
    fn synthesize<'a>(&'a self, text: &str, style_id: u32)
        -> Result<Self::Output<'a>, Self::Error>;
    /// Returns speaker metadata currently visible to the core instance.
    ///
    /// # Errors
    ///
    /// Returns an implementation-specific error if metadata retrieval fails.
    fn get_speakers(&self) -> Result<Self::SpeakerData<'_>, Self::Error>;
}

pub struct VoicevoxCore {
    synthesizer: Synthesizer<OpenJtalk>,
}

impl VoicevoxCore {
    /// Creates a `VoicevoxCore` instance and initializes ONNX Runtime/OpenJTalk.
    ///
    /// # Errors
    ///
    /// Returns an error if runtime libraries, dictionary resources, or the synthesizer
    /// builder cannot be initialized.
    pub fn new() -> Result<Self> {
        let onnxruntime = onnxruntime::initialize()?;
        let open_jtalk = openjtalk::initialize()?;

        let synthesizer = Synthesizer::builder(onnxruntime)
            .text_analyzer(open_jtalk)
            .acceleration_mode(AccelerationMode::Cpu)
            .cpu_num_threads(0)
            .build()
            .map_err(|e| anyhow!("Failed to create synthesizer: {e}"))?;

        Ok(Self { synthesizer })
    }

    /// Synthesizes speech while applying a speech-rate multiplier via `AudioQuery`.
    ///
    /// # Errors
    ///
    /// Returns an error if text is empty, rate is outside the supported range, or
    /// query generation/synthesis fails.
    pub fn synthesize_with_rate(&self, text: &str, style_id: u32, rate: f32) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            return Err(anyhow!("Empty text provided for synthesis"));
        }

        if !is_valid_synthesis_rate(rate) {
            return Err(anyhow!(
                "Rate must be between {MIN_SYNTHESIS_RATE:.1} and {MAX_SYNTHESIS_RATE:.1}, got: {rate}"
            ));
        }

        let style_id = StyleId::new(style_id);
        let mut query = self
            .synthesizer
            .create_audio_query(text, style_id)
            .map_err(|e| anyhow!("Failed to create audio query: {e}"))?;
        query.speed_scale = rate;

        self.synthesizer
            .synthesis(&query, style_id)
            .perform()
            .map_err(|e| anyhow!("Speech synthesis failed: {e}"))
    }
}

impl CoreSynthesis for VoicevoxCore {
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
        self.synthesize_with_rate(text, style_id, DEFAULT_SYNTHESIS_RATE)
            .map_err(|e| anyhow!("Speech synthesis failed for style_id {style_id}: {e}"))
    }

    fn get_speakers(&self) -> Result<Self::SpeakerData<'_>, Self::Error> {
        Ok(crate::infrastructure::voicevox::collect_speakers_from_synthesizer(&self.synthesizer))
    }
}

impl VoicevoxCore {
    /// Loads a specific `.vvm` voice model by numeric model ID (e.g. `3` => `3.vvm`).
    ///
    /// # Errors
    ///
    /// Returns an error if the model directory cannot be found, the model file does not
    /// exist, or the core fails to load the model.
    pub fn load_specific_model(&self, model_id: u32) -> Result<()> {
        let model = open_voice_model_file_by_id(model_id)?;

        match self.synthesizer.load_voice_model(&model) {
            Ok(()) => Ok(()),
            Err(error) if is_already_loaded_error(&error.to_string()) => Ok(()),
            Err(error) => Err(anyhow!("Failed to load model {model_id}: {error}")),
        }
    }

    /// Unloads a voice model by file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the model file cannot be opened or the core fails to unload it.
    pub fn unload_voice_model_by_path(&self, model_path: &Path) -> Result<()> {
        let model = open_voice_model_file(model_path)?;
        let voice_model_id = model.id();

        self.synthesizer
            .unload_voice_model(voice_model_id)
            .map_err(|e| anyhow!("Failed to unload model: {e}"))
    }
}

fn is_already_loaded_error(message: &str) -> bool {
    message.contains("既に読み込まれています")
        || message.to_ascii_lowercase().contains("already loaded")
}
