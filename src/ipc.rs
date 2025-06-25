//! Inter-process communication protocols and data structures
//!
//! This module defines the binary protocol for daemon-client communication using
//! Unix sockets with bincode serialization. Provides type-safe, efficient IPC
//! for voice synthesis operations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::voice::Speaker;

/// Request messages sent from client to daemon
///
/// Defines all operations that clients can request from the daemon,
/// including synthesis, speaker listing, and model management.
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