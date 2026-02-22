use anyhow::{anyhow, Result};
use std::path::Path;
use voicevox_core::{
    blocking::{Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile},
    AccelerationMode, StyleId,
};

use crate::paths::{find_models_dir, find_onnxruntime, find_openjtalk_dict};
use crate::voice::Speaker;

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
        let onnxruntime = find_onnxruntime()
            .map_or_else(
                |_| Onnxruntime::load_once().perform(),
                |ort_path| Onnxruntime::load_once().filename(ort_path).perform(),
            )
            .map_err(|_| {
                anyhow!(
                    "Failed to initialize ONNX Runtime. Please run 'voicevox-setup' to download required resources."
                )
            })?;

        let dict_path = find_openjtalk_dict()?;

        let open_jtalk = OpenJtalk::new(
            dict_path
                .to_str()
                .ok_or_else(|| anyhow!("Invalid OpenJTalk dictionary path"))?,
        )
        .map_err(|e| anyhow!("Failed to initialize OpenJTalk: {e}"))?;

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

        if !(0.5..=2.0).contains(&rate) {
            return Err(anyhow!("Rate must be between 0.5 and 2.0, got: {rate}"));
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
        self.synthesize_with_rate(text, style_id, 1.0)
            .map_err(|e| anyhow!("Speech synthesis failed for style_id {style_id}: {e}"))
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

impl VoicevoxCore {
    /// Loads a specific `.vvm` voice model by model file stem (e.g. `"3"`).
    ///
    /// # Errors
    ///
    /// Returns an error if the model directory cannot be found, the model file does not
    /// exist, or the core fails to load the model.
    pub fn load_specific_model(&self, model_name: &str) -> Result<()> {
        let models_dir = find_models_dir()?;
        let model_path = models_dir.join(format!("{model_name}.vvm"));

        if !model_path.exists() {
            return Err(anyhow!(
                "Model not found: {model_name}.vvm at {}",
                models_dir.display()
            ));
        }

        let model = VoiceModelFile::open(&model_path)
            .map_err(|e| anyhow!("Failed to open model {model_name}: {e}"))?;

        self.synthesizer
            .load_voice_model(&model)
            .map_err(|e| anyhow!("Failed to load model {model_name}: {e}"))
    }

    /// Unloads a voice model by file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the model file cannot be opened or the core fails to unload it.
    pub fn unload_voice_model_by_path(&self, model_path: &Path) -> Result<()> {
        let model = VoiceModelFile::open(model_path)
            .map_err(|e| anyhow!("Failed to open model file {}: {e}", model_path.display()))?;
        let voice_model_id = model.id();

        self.synthesizer
            .unload_voice_model(voice_model_id)
            .map_err(|e| anyhow!("Failed to unload model: {e}"))
    }
}
