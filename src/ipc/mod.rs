use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::voice::Speaker;

// IPC Protocol Definitions
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonRequest {
    Ping,
    Synthesize {
        text: String,
        style_id: u32,
        options: SynthesizeOptions,
    },
    ListSpeakers,
    LoadModel {
        model_name: String,
    },
    GetVoiceMapping,
    ResolveVoiceName {
        voice_name: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynthesizeOptions {
    pub rate: f32,
    pub streaming: bool,
}

impl Default for SynthesizeOptions {
    fn default() -> Self {
        Self {
            rate: 1.0,
            streaming: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
    Pong,
    SynthesizeResult {
        wav_data: Vec<u8>,
    },
    SpeakersList {
        speakers: Vec<Speaker>,
    },
    VoiceMapping {
        mapping: HashMap<String, (u32, String)>,
    },
    VoiceResolution {
        style_id: u32,
        description: String,
    },
    Success,
    Error {
        message: String,
    },
}