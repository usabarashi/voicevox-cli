use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::voice::{AvailableModel, Speaker};

pub const DEFAULT_SYNTHESIS_RATE: f32 = 1.0;
pub const MIN_SYNTHESIS_RATE: f32 = 0.5;
pub const MAX_SYNTHESIS_RATE: f32 = 2.0;
pub const DAEMON_IPC_PROTOCOL_VERSION: u32 = 1;

#[must_use]
pub const fn is_valid_synthesis_rate(rate: f32) -> bool {
    rate >= MIN_SYNTHESIS_RATE && rate <= MAX_SYNTHESIS_RATE
}

/// Request messages sent from client to daemon
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonRequest {
    Ping,
    GetServerInfo,
    Synthesize {
        text: String,
        style_id: u32,
        options: SynthesizeOptions,
    },
    ListSpeakers,
    ListModels,
}

/// Synthesis options for voice synthesis requests
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct SynthesizeOptions {
    pub rate: f32,
}

impl Default for SynthesizeOptions {
    fn default() -> Self {
        Self {
            rate: DEFAULT_SYNTHESIS_RATE,
        }
    }
}

/// Response messages from daemon to client
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
    Pong,
    ServerInfo {
        protocol_version: u32,
        daemon_version: String,
        capabilities: Vec<String>,
    },
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
    ModelsList {
        models: Vec<AvailableModel>,
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
