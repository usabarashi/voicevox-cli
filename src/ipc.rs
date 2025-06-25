//! Inter-process communication protocols and data structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::borrow::Cow;

use crate::voice::Speaker;

/// IPC configuration with compile-time buffer size optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcConfig<const BUFFER_SIZE: usize = 65536, const MAX_TEXT_LENGTH: usize = 8192>;

impl<const BUFFER_SIZE: usize, const MAX_TEXT_LENGTH: usize> IpcConfig<BUFFER_SIZE, MAX_TEXT_LENGTH> {
    pub const fn new() -> Self {
        Self
    }
    
    pub const fn buffer_size() -> usize {
        BUFFER_SIZE
    }
    
    pub const fn max_text_length() -> usize {
        MAX_TEXT_LENGTH
    }
    
    pub const fn validate_text_length(text_len: usize) -> bool {
        text_len <= MAX_TEXT_LENGTH
    }
}

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
    LoadModel {
        model_name: Cow<'a, str>,
    },
    GetVoiceMapping,
    ResolveVoiceName {
        voice_name: Cow<'a, str>,
    },
}

impl<'a> DaemonRequest<'a> {
    pub const fn ping() -> Self {
        Self::Ping
    }
    
    pub fn synthesize_borrowed(text: &'a str, style_id: u32, options: SynthesizeOptions<'a>) -> Self {
        Self::Synthesize {
            text: Cow::Borrowed(text),
            style_id,
            options,
        }
    }
    
    pub fn synthesize_owned(text: String, style_id: u32, options: SynthesizeOptions<'a>) -> Self {
        Self::Synthesize {
            text: Cow::Owned(text),
            style_id,
            options,
        }
    }
    
    pub fn load_model_borrowed(model_name: &'a str) -> Self {
        Self::LoadModel {
            model_name: Cow::Borrowed(model_name),
        }
    }
    
    pub fn resolve_voice_borrowed(voice_name: &'a str) -> Self {
        Self::ResolveVoiceName {
            voice_name: Cow::Borrowed(voice_name),
        }
    }
}

/// Synthesis options for voice synthesis requests
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynthesizeOptions<'a> {
    pub rate: f32,
    pub streaming: bool,
    pub context: Option<Cow<'a, str>>,
}

impl<'a> SynthesizeOptions<'a> {
    pub const fn default_const() -> Self {
        Self {
            rate: 1.0,
            streaming: false,
            context: None,
        }
    }
    
    pub fn with_context_borrowed(rate: f32, streaming: bool, context: &'a str) -> Self {
        Self {
            rate,
            streaming,
            context: Some(Cow::Borrowed(context)),
        }
    }
    
    pub const fn validate_rate(rate: f32) -> bool {
        rate >= 0.1 && rate <= 3.0
    }
    pub fn validated(rate: f32, streaming: bool) -> Result<Self, &'static str> {
        if !Self::validate_rate(rate) {
            return Err("Rate must be between 0.1 and 3.0");
        }
        
        Ok(Self {
            rate,
            streaming,
            context: None,
        })
    }
}

impl<'a> Default for SynthesizeOptions<'a> {
    fn default() -> Self {
        Self {
            rate: 1.0,
            streaming: false,
            context: None,
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
    VoiceMapping {
        mapping: HashMap<Cow<'a, str>, (u32, Cow<'a, str>)>,
    },
    VoiceResolution {
        style_id: u32,
        description: Cow<'a, str>,
    },
    Success,
    Error {
        message: Cow<'a, str>,
    },
}

impl<'a> DaemonResponse<'a> {
    pub const fn pong() -> Self {
        Self::Pong
    }
    
    pub const fn success() -> Self {
        Self::Success
    }
    
    pub fn synthesize_result_borrowed(wav_data: &'a [u8]) -> Self {
        Self::SynthesizeResult {
            wav_data: Cow::Borrowed(wav_data),
        }
    }
    
    pub fn synthesize_result_owned(wav_data: Vec<u8>) -> Self {
        Self::SynthesizeResult {
            wav_data: Cow::Owned(wav_data),
        }
    }
    
    pub fn speakers_list_borrowed(speakers: &'a [Speaker]) -> Self {
        Self::SpeakersList {
            speakers: Cow::Borrowed(speakers),
        }
    }
    
    pub fn error_borrowed(message: &'a str) -> Self {
        Self::Error {
            message: Cow::Borrowed(message),
        }
    }
    
    pub fn error_owned(message: String) -> Self {
        Self::Error {
            message: Cow::Owned(message),
        }
    }
    
    pub fn voice_resolution_borrowed(style_id: u32, description: &'a str) -> Self {
        Self::VoiceResolution {
            style_id,
            description: Cow::Borrowed(description),
        }
    }
}

/// Request type for owned data
pub type OwnedRequest = DaemonRequest<'static>;

/// Response type for owned data
pub type OwnedResponse = DaemonResponse<'static>;

/// Synthesis options for owned data
pub type OwnedSynthesizeOptions = SynthesizeOptions<'static>;