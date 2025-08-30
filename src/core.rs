use anyhow::{anyhow, Result};
use voicevox_core::{
    blocking::{Onnxruntime, OpenJtalk, Synthesizer, VoiceModelFile},
    AccelerationMode,
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

    fn synthesize<'a>(&'a self, text: &str, style_id: u32)
        -> Result<Self::Output<'a>, Self::Error>;
    fn get_speakers(&self) -> Result<Self::SpeakerData<'_>, Self::Error>;
}

pub struct VoicevoxCore {
    synthesizer: Synthesizer<OpenJtalk>,
}

impl VoicevoxCore {
    pub fn new() -> Result<Self> {
        let onnxruntime = if let Ok(ort_path) = find_onnxruntime() {
            Onnxruntime::load_once()
                .filename(ort_path)
                .perform()
        } else {
            Onnxruntime::load_once()
                .perform()
        }.map_err(|_| anyhow!("Failed to initialize ONNX Runtime. Please run 'voicevox-setup' to download required resources."))?;

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

        Ok(VoicevoxCore { synthesizer })
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
        use voicevox_core::StyleId;

        if text.trim().is_empty() {
            return Err(anyhow!("Empty text provided for synthesis"));
        }

        self.synthesizer
            .tts(text, StyleId::new(style_id))
            .perform()
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

    pub fn unload_voice_model_by_path(&self, model_path: &str) -> Result<()> {
        let model = VoiceModelFile::open(model_path)
            .map_err(|e| anyhow!("Failed to open model file: {e}"))?;
        let voice_model_id = model.id();

        self.synthesizer
            .unload_voice_model(voice_model_id)
            .map_err(|e| anyhow!("Failed to unload model: {e}"))
    }
}
