//! Inter-process communication protocols and data structures

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;

use crate::voice::Speaker;

/// Audio format information for zero-copy transfer
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 24000,
            channels: 1,
            bits_per_sample: 16,
        }
    }
}

/// Protocol capabilities for feature negotiation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProtocolCapabilities {
    /// Protocol version
    pub version: u32,
    /// Supports file descriptor passing for zero-copy
    #[cfg(unix)]
    pub supports_fd_passing: bool,
    /// Supports chunked streaming
    pub supports_streaming: bool,
    /// Supports shared memory segments
    pub supports_shared_memory: bool,
}

impl Default for ProtocolCapabilities {
    fn default() -> Self {
        Self {
            version: 1,
            #[cfg(unix)]
            supports_fd_passing: true,
            supports_streaming: true,
            supports_shared_memory: false,
        }
    }
}

/// IPC configuration with compile-time buffer size optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpcConfig<const BUFFER_SIZE: usize = 65536, const MAX_TEXT_LENGTH: usize = 8192>;

impl<const BUFFER_SIZE: usize, const MAX_TEXT_LENGTH: usize> Default
    for IpcConfig<BUFFER_SIZE, MAX_TEXT_LENGTH>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const BUFFER_SIZE: usize, const MAX_TEXT_LENGTH: usize>
    IpcConfig<BUFFER_SIZE, MAX_TEXT_LENGTH>
{
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
    /// Get protocol capabilities
    GetCapabilities,
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

    pub fn synthesize_borrowed(
        text: &'a str,
        style_id: u32,
        options: SynthesizeOptions<'a>,
    ) -> Self {
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
    /// Request zero-copy transfer if supported
    #[serde(default)]
    pub zero_copy: bool,
}

impl<'a> SynthesizeOptions<'a> {
    pub const fn default_const() -> Self {
        Self {
            rate: 1.0,
            streaming: false,
            context: None,
            zero_copy: false,
        }
    }

    pub fn with_context_borrowed(rate: f32, streaming: bool, context: &'a str) -> Self {
        Self {
            rate,
            streaming,
            context: Some(Cow::Borrowed(context)),
            zero_copy: false,
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
            zero_copy: false,
        })
    }
}

impl Default for SynthesizeOptions<'_> {
    fn default() -> Self {
        Self {
            rate: 1.0,
            streaming: false,
            context: None,
            zero_copy: false,
        }
    }
}

/// Response messages from daemon to client
#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse<'a> {
    Pong,
    /// Protocol capabilities response
    Capabilities(ProtocolCapabilities),
    SynthesizeResult {
        wav_data: Cow<'a, [u8]>,
    },
    /// Zero-copy audio response using file descriptor
    #[cfg(unix)]
    SynthesizeResultFd {
        size: usize,
        format: AudioFormat,
    },
    /// Streaming audio response - header with total size
    SynthesizeStreamHeader {
        total_size: usize,
        chunk_size: usize,
    },
    /// Streaming audio response - data chunk
    SynthesizeStreamChunk {
        chunk_id: u32,
        data: Cow<'a, [u8]>,
        is_final: bool,
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

    pub fn stream_header(total_size: usize, chunk_size: usize) -> Self {
        Self::SynthesizeStreamHeader {
            total_size,
            chunk_size,
        }
    }

    pub fn stream_chunk_borrowed(chunk_id: u32, data: &'a [u8], is_final: bool) -> Self {
        Self::SynthesizeStreamChunk {
            chunk_id,
            data: Cow::Borrowed(data),
            is_final,
        }
    }

    pub fn stream_chunk_owned(chunk_id: u32, data: Vec<u8>, is_final: bool) -> Self {
        Self::SynthesizeStreamChunk {
            chunk_id,
            data: Cow::Owned(data),
            is_final,
        }
    }
}

/// Request type for owned data
pub type OwnedRequest = DaemonRequest<'static>;

/// Response type for owned data
pub type OwnedResponse = DaemonResponse<'static>;

/// Synthesis options for owned data
pub type OwnedSynthesizeOptions = SynthesizeOptions<'static>;
