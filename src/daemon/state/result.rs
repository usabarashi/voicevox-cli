use std::collections::HashMap;

use crate::voice::{AvailableModel, Speaker};

pub(super) enum DaemonServiceResult {
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
}

#[derive(Debug, Clone, Copy)]
pub(super) enum DaemonServiceErrorKind {
    InvalidTargetId,
    ModelLoadFailed,
    SynthesisFailed,
}

pub(super) struct DaemonServiceError {
    pub(super) kind: DaemonServiceErrorKind,
    pub(super) message: String,
}

impl DaemonServiceError {
    pub(super) fn new(kind: DaemonServiceErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
