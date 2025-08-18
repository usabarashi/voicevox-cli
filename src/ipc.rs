use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::voice::Speaker;

/// Request messages sent from client to daemon
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonRequest {
    Ping,
    Synthesize {
        text: String,
        style_id: u32,
        options: SynthesizeOptions,
    },
    ListSpeakers,
}

/// Synthesis options for voice synthesis requests
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynthesizeOptions {
    pub rate: f32,
}

impl Default for SynthesizeOptions {
    fn default() -> Self {
        Self { rate: 1.0 }
    }
}

/// Response messages from daemon to client
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
    Pong,
    SynthesizeResult {
        wav_data: Vec<u8>,
    },
    SpeakersList {
        speakers: Vec<Speaker>,
    },
    /// Enhanced speakers list with model ID mapping
    SpeakersListWithModels {
        speakers: Vec<Speaker>,
        style_to_model: HashMap<u32, u32>,
    },
    Error {
        message: String,
    },
}

/// Request type for owned data
pub type OwnedRequest = DaemonRequest;

/// Response type for owned data
pub type OwnedResponse = DaemonResponse;

/// Synthesis options for owned data
pub type OwnedSynthesizeOptions = SynthesizeOptions;
