use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "audio")]
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

fn text_content(text: impl Into<String>) -> ToolContent {
    ToolContent::Text { text: text.into() }
}

fn audio_content(wav_data: &[u8]) -> ToolContent {
    ToolContent::Audio {
        data: base64::engine::general_purpose::STANDARD.encode(wav_data),
        mime_type: "audio/wav".to_string(),
    }
}

pub(crate) fn text_result(text: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult {
        content: vec![text_content(text)],
        is_error: is_error.then_some(true),
    }
}

pub(crate) fn audio_result(summary: impl Into<String>, wav_data: &[u8]) -> ToolCallResult {
    ToolCallResult {
        content: vec![text_content(summary), audio_content(wav_data)],
        is_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn audio_result_uses_audio_content_shape() {
        let result = audio_result("ok", b"RIFF");
        let value = serde_json::to_value(result).expect("tool result should serialize");

        assert_eq!(value["content"][0], json!({"type": "text", "text": "ok"}));
        assert_eq!(value["content"][1]["type"], "audio");
        assert_eq!(value["content"][1]["mimeType"], "audio/wav");
        assert!(value["content"][1]["data"].as_str().is_some());
    }
}
