use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::voice::{AvailableModel, Speaker};

pub const DEFAULT_SYNTHESIS_RATE: f32 = 1.0;
pub const MIN_SYNTHESIS_RATE: f32 = 0.5;
pub const MAX_SYNTHESIS_RATE: f32 = 2.0;
pub const MAX_SYNTHESIS_TEXT_LENGTH: usize = 10_000;
pub const MAX_DAEMON_REQUEST_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_DAEMON_RESPONSE_FRAME_BYTES: usize = 128 * 1024 * 1024;
#[must_use]
pub const fn is_valid_synthesis_rate(rate: f32) -> bool {
    rate >= MIN_SYNTHESIS_RATE && rate <= MAX_SYNTHESIS_RATE
}

/// Request messages sent from client to daemon
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonRequest {
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
    SynthesizeResult {
        wav_data: Vec<u8>,
    },
    SpeakersListWithModels {
        speakers: Vec<Speaker>,
        style_to_model: HashMap<u32, u32>,
    },
    ModelsList {
        models: Vec<AvailableModel>,
    },
    Error {
        code: DaemonErrorCode,
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DaemonErrorCode {
    InvalidTargetId,
    ModelLoadFailed,
    SynthesisFailed,
    Internal,
}

/// Request type for owned data
pub type OwnedRequest = DaemonRequest;

/// Response type for owned data
pub type OwnedResponse = DaemonResponse;

/// Synthesis options for owned data
pub type OwnedSynthesizeOptions = SynthesizeOptions;
