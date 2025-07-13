use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::voice::Speaker;

/// Request messages sent from client to daemon
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonRequest<'a> {
    Ping,
    Synthesize {
        text: Cow<'a, str>,
        style_id: u32,
        options: SynthesizeOptions<'a>,
    },
    ListSpeakers,
}

/// Synthesis options for voice synthesis requests
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynthesizeOptions<'a> {
    pub rate: f32,
    #[serde(skip)]
    pub _phantom: std::marker::PhantomData<&'a ()>,
}

impl Default for SynthesizeOptions<'_> {
    fn default() -> Self {
        Self {
            rate: 1.0,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Response messages from daemon to client
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse<'a> {
    Pong,
    SynthesizeResult {
        wav_data: Cow<'a, [u8]>,
    },
    SpeakersList {
        speakers: Cow<'a, [Speaker]>,
    },
    /// Enhanced speakers list with model ID mapping
    SpeakersListWithModels {
        speakers: Cow<'a, [Speaker]>,
        style_to_model: HashMap<u32, u32>,
    },
    Success,
    Error {
        message: Cow<'a, str>,
    },
}

/// Request type for owned data
pub type OwnedRequest = DaemonRequest<'static>;

/// Response type for owned data
pub type OwnedResponse = DaemonResponse<'static>;

/// Synthesis options for owned data
pub type OwnedSynthesizeOptions = SynthesizeOptions<'static>;
