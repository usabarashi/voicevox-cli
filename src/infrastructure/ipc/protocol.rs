use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::DEFAULT_SYNTHESIS_RATE;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct IpcStyle {
    pub name: String,
    pub id: u32,
    #[serde(rename = "type")]
    pub style_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct IpcSpeaker {
    pub name: String,
    #[serde(default)]
    pub speaker_uuid: String,
    pub styles: Vec<IpcStyle>,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct IpcModel {
    pub model_id: u32,
    pub file_path: std::path::PathBuf,
    pub speakers: Vec<IpcSpeaker>,
}

/// Request messages sent from client to daemon.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, PartialEq)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn roundtrip_request(request: &DaemonRequest) -> DaemonRequest {
        let encoded = postcard::to_allocvec(request).expect("encode request");
        postcard::from_bytes(&encoded).expect("decode request")
    }

    fn roundtrip_response(response: &DaemonResponse) -> DaemonResponse {
        let encoded = postcard::to_allocvec(response).expect("encode response");
        postcard::from_bytes(&encoded).expect("decode response")
    }

    #[test]
    fn synthesize_request_roundtrip() {
        let request = DaemonRequest::Synthesize {
            text: "これはテストです".to_string(),
            style_id: 3,
            options: SynthesizeOptions { rate: 1.2 },
        };
        assert_eq!(roundtrip_request(&request), request);
    }

    #[test]
    fn unit_variant_requests_roundtrip() {
        assert_eq!(
            roundtrip_request(&DaemonRequest::ListSpeakers),
            DaemonRequest::ListSpeakers
        );
        assert_eq!(
            roundtrip_request(&DaemonRequest::ListModels),
            DaemonRequest::ListModels
        );
    }

    #[test]
    fn synthesize_result_preserves_wav_bytes() {
        let wav_data: Vec<u8> = (0..65536).map(|i| (i % 256) as u8).collect();
        let response = DaemonResponse::SynthesizeResult {
            wav_data: wav_data.clone(),
        };
        let decoded = roundtrip_response(&response);
        assert_eq!(decoded, response);
        if let DaemonResponse::SynthesizeResult {
            wav_data: decoded_wav,
        } = decoded
        {
            assert_eq!(decoded_wav.len(), 65536);
            assert_eq!(decoded_wav, wav_data);
        } else {
            panic!("expected SynthesizeResult");
        }
    }

    #[test]
    fn speakers_list_with_models_roundtrip() {
        let response = DaemonResponse::SpeakersListWithModels {
            speakers: vec![IpcSpeaker {
                name: "ずんだもん".to_string(),
                speaker_uuid: "uuid-1234".to_string(),
                styles: vec![
                    IpcStyle {
                        name: "ノーマル".to_string(),
                        id: 3,
                        style_type: Some("talk".to_string()),
                    },
                    IpcStyle {
                        name: "あまあま".to_string(),
                        id: 1,
                        style_type: None,
                    },
                ],
                version: "0.1.0".to_string(),
            }],
            style_to_model: HashMap::from([(3, 0), (1, 0)]),
        };
        assert_eq!(roundtrip_response(&response), response);
    }

    #[test]
    fn models_list_roundtrip() {
        let response = DaemonResponse::ModelsList {
            models: vec![IpcModel {
                model_id: 0,
                file_path: PathBuf::from("/path/to/0.vvm"),
                speakers: vec![],
            }],
        };
        assert_eq!(roundtrip_response(&response), response);
    }

    #[test]
    fn error_response_roundtrip() {
        let response = DaemonResponse::Error {
            code: DaemonErrorCode::SynthesisFailed,
            message: "synthesis error".to_string(),
        };
        assert_eq!(roundtrip_response(&response), response);
    }
}
