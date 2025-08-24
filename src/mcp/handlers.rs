use anyhow::{anyhow, Context, Result};
use rodio::{OutputStream, Sink};
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::client::{audio::play_audio_from_memory, DaemonClient};
use crate::mcp::types::{ToolCallResult, ToolContent};
use crate::synthesis::StreamingSynthesizer;

const MAX_STYLE_ID: u32 = 1000;

#[derive(Error, Debug)]
pub enum SynthesisError {
    #[error("Text cannot be empty")]
    EmptyText,

    #[error("Text too long: {length} characters (max: {max})")]
    TextTooLong { length: usize, max: usize },

    #[error("Rate must be between 0.5 and 2.0")]
    InvalidRate,

    #[error("Invalid style_id: {style_id} (max: {max})")]
    InvalidStyleId { style_id: u32, max: u32 },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Deserialize)]
struct SynthesizeParams {
    text: String,
    style_id: u32,
    #[serde(default = "default_rate")]
    rate: f32,
    #[serde(default = "default_streaming")]
    streaming: bool,
}

fn default_rate() -> f32 {
    1.0
}

fn default_streaming() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct ListVoiceStylesParams {
    speaker_name: Option<String>,
    style_name: Option<String>,
}

pub async fn handle_text_to_speech(arguments: Value) -> Result<ToolCallResult, SynthesisError> {
    let params: SynthesizeParams = serde_json::from_value(arguments).map_err(|e| {
        SynthesisError::Other(anyhow!("Invalid parameters for text_to_speech: {}", e))
    })?;

    let text = params.text.trim();
    if text.is_empty() {
        return Err(SynthesisError::EmptyText);
    }

    const MAX_TEXT_LENGTH: usize = 10_000;
    if text.len() > MAX_TEXT_LENGTH {
        return Err(SynthesisError::TextTooLong {
            length: text.len(),
            max: MAX_TEXT_LENGTH,
        });
    }

    if !(0.5..=2.0).contains(&params.rate) {
        return Err(SynthesisError::InvalidRate);
    }

    if params.style_id > MAX_STYLE_ID {
        return Err(SynthesisError::InvalidStyleId {
            style_id: params.style_id,
            max: MAX_STYLE_ID,
        });
    }

    if params.streaming {
        handle_streaming_synthesis(params)
            .await
            .map_err(SynthesisError::Other)
    } else {
        handle_daemon_synthesis(params)
            .await
            .map_err(SynthesisError::Other)
    }
}

async fn handle_streaming_synthesis(params: SynthesizeParams) -> Result<ToolCallResult> {
    let (_stream, stream_handle) =
        OutputStream::try_default().context("Failed to create audio output stream")?;
    let sink = Sink::try_new(&stream_handle).context("Failed to create audio sink")?;

    let mut synthesizer = StreamingSynthesizer::new()
        .await
        .context("Failed to create streaming synthesizer")?;

    synthesizer
        .synthesize_streaming(&params.text, params.style_id, params.rate, &sink)
        .await
        .context("Streaming synthesis failed")?;

    sink.sleep_until_end();
    drop(_stream);

    Ok(ToolCallResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: format!(
                "Successfully synthesized {} characters using style ID {} in streaming mode",
                params.text.len(),
                params.style_id
            ),
        }],
        is_error: Some(false),
    })
}

async fn handle_daemon_synthesis(params: SynthesizeParams) -> Result<ToolCallResult> {
    // Try to connect with retries
    let mut client = match DaemonClient::connect_with_retry().await {
        Ok(client) => client,
        Err(e) => {
            return Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: format!("Failed to connect to VOICEVOX daemon: {e}"),
                }],
                is_error: Some(true),
            });
        }
    };

    let options = crate::ipc::OwnedSynthesizeOptions {
        rate: params.rate,
        ..Default::default()
    };

    let wav_data = client
        .synthesize(&params.text, params.style_id, options)
        .await
        .context("Synthesis failed")?;

    play_audio_from_memory(&wav_data).context("Failed to play audio")?;

    Ok(ToolCallResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: format!(
                "Successfully synthesized {} characters using style ID {} (audio size: {} bytes)",
                params.text.len(),
                params.style_id,
                wav_data.len()
            ),
        }],
        is_error: Some(false),
    })
}

pub async fn handle_list_voice_styles(arguments: Value) -> Result<ToolCallResult> {
    let params: ListVoiceStylesParams =
        serde_json::from_value(arguments).context("Invalid parameters for list_voice_styles")?;

    let mut client = DaemonClient::connect_with_retry()
        .await
        .context("Failed to connect to VOICEVOX daemon after multiple attempts")?;

    let speakers = client.list_speakers().await?;

    let mut filtered_results = Vec::new();

    for speaker in speakers {
        if let Some(name_filter) = &params.speaker_name {
            if !speaker
                .name
                .to_lowercase()
                .contains(&name_filter.to_lowercase())
            {
                continue;
            }
        }

        let filtered_styles = if let Some(style_filter) = &params.style_name {
            speaker
                .styles
                .into_iter()
                .filter(|style| {
                    style
                        .name
                        .to_lowercase()
                        .contains(&style_filter.to_lowercase())
                })
                .collect::<Vec<_>>()
        } else {
            speaker.styles.to_vec()
        };

        if !filtered_styles.is_empty() {
            filtered_results.push((speaker.name, filtered_styles));
        }
    }

    let mut result_text = String::new();
    if filtered_results.is_empty() {
        result_text.push_str("No speakers found matching the criteria.");
    } else {
        for (speaker_name, styles) in &filtered_results {
            result_text.push_str(&format!("Speaker: {}\n", speaker_name));
            result_text.push_str("Styles:\n");
            for style in styles {
                result_text.push_str(&format!("  - {} (ID: {})\n", style.name, style.id));
            }
            result_text.push('\n');
        }
        result_text.push_str(&format!("Total speakers found: {}", filtered_results.len()));
    }
    Ok(ToolCallResult {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: result_text.trim().to_string(),
        }],
        is_error: Some(false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_text_to_speech_empty_text() {
        let args = json!({
            "text": "",
            "style_id": 3,
            "streaming": false
        });

        let result = handle_text_to_speech(args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SynthesisError::EmptyText));
    }

    #[tokio::test]
    async fn test_text_to_speech_text_too_long() {
        let long_text = "あ".repeat(10_001);
        let args = json!({
            "text": long_text,
            "style_id": 3,
            "streaming": false
        });

        let result = handle_text_to_speech(args).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SynthesisError::TextTooLong { .. }
        ));
    }

    #[tokio::test]
    async fn test_text_to_speech_invalid_rate() {
        let args = json!({
            "text": "テスト",
            "style_id": 3,
            "rate": 3.0,
            "streaming": false
        });

        let result = handle_text_to_speech(args).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SynthesisError::InvalidRate));
    }

    #[tokio::test]
    async fn test_text_to_speech_invalid_style_id() {
        let args = json!({
            "text": "テスト",
            "style_id": MAX_STYLE_ID + 1,
            "streaming": false
        });

        let result = handle_text_to_speech(args).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SynthesisError::InvalidStyleId { .. }
        ));
    }
}
