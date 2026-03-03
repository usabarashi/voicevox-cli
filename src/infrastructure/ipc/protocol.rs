use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::DEFAULT_SYNTHESIS_RATE;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IpcStyle {
    pub name: String,
    pub id: u32,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub style_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IpcSpeaker {
    pub name: String,
    #[serde(default)]
    pub speaker_uuid: String,
    pub styles: Vec<IpcStyle>,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IpcModel {
    pub model_id: u32,
    pub file_path: std::path::PathBuf,
    pub speakers: Vec<IpcSpeaker>,
}

/// Request messages sent from client to daemon.
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

/// Synthesis options for voice synthesis requests.
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

/// Response messages from daemon to client.
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
    SynthesizeResult {
        wav_data: Vec<u8>,
    },
    SpeakersListWithModels {
        speakers: Vec<IpcSpeaker>,
        style_to_model: HashMap<u32, u32>,
    },
    ModelsList {
        models: Vec<IpcModel>,
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

/// Request type for owned data.
pub type OwnedRequest = DaemonRequest;

/// Response type for owned data.
pub type OwnedResponse = DaemonResponse;

/// Synthesis options for owned data.
pub type OwnedSynthesizeOptions = SynthesizeOptions;
