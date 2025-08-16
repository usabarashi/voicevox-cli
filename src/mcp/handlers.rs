use anyhow::{anyhow, Context, Result};
use rodio::{OutputStream, Sink};
use serde::Deserialize;
use serde_json::Value;

use crate::client::{audio::play_audio_from_memory, DaemonClient};
use crate::mcp::types::{ToolCallResult, ToolContent};
use crate::synthesis::StreamingSynthesizer;
use crate::voice::Speaker;

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

pub async fn handle_text_to_speech(arguments: Value) -> Result<ToolCallResult> {
    let params: SynthesizeParams =
        serde_json::from_value(arguments).context("Invalid parameters for text_to_speech")?;

    let text = params.text.trim();
    (!text.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!("Text cannot be empty"))?;

    const MAX_TEXT_LENGTH: usize = 10_000;
    (text.len() <= MAX_TEXT_LENGTH)
        .then_some(())
        .ok_or_else(|| {
            anyhow!(
                "Text too long: {} characters (max: {})",
                text.len(),
                MAX_TEXT_LENGTH
            )
        })?;

    (0.5..=2.0)
        .contains(&params.rate)
        .then_some(())
        .ok_or_else(|| anyhow!("Rate must be between 0.5 and 2.0"))?;

    (params.style_id <= 1000)
        .then_some(())
        .ok_or_else(|| anyhow!("Invalid style_id: {}", params.style_id))?;

    if params.streaming {
        handle_streaming_synthesis(params).await
    } else {
        handle_daemon_synthesis(params).await
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

async fn connect_with_retry() -> Result<DaemonClient> {
    use crate::daemon::startup;

    let mut last_error = None;
    let mut retry_delay = startup::initial_retry_delay();

    for attempt in 0..startup::MAX_CONNECT_ATTEMPTS {
        match DaemonClient::new().await {
            Ok(client) => return Ok(client),
            Err(e) => {
                last_error = Some(e);
                if attempt < startup::MAX_CONNECT_ATTEMPTS - 1 {
                    tokio::time::sleep(retry_delay).await;
                    retry_delay = (retry_delay * 2).min(startup::max_retry_delay());
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow!(
            "Failed to connect to daemon after {} attempts",
            startup::MAX_CONNECT_ATTEMPTS
        )
    }))
}

async fn handle_daemon_synthesis(params: SynthesizeParams) -> Result<ToolCallResult> {
    // Try to connect with retries
    let mut client = match connect_with_retry().await {
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

    let mut client = DaemonClient::new()
        .await
        .context("Failed to connect to VOICEVOX daemon. Is it running?")?;

    let speakers = client.list_speakers().await?;

    let filtered_speakers: Vec<Speaker> = speakers
        .into_iter()
        .filter(|speaker| match &params.speaker_name {
            Some(name_filter) => speaker
                .name
                .to_lowercase()
                .contains(&name_filter.to_lowercase()),
            None => true,
        })
        .filter(|speaker| match &params.style_name {
            Some(style_filter) => speaker.styles.iter().any(|style| {
                style
                    .name
                    .to_lowercase()
                    .contains(&style_filter.to_lowercase())
            }),
            None => true,
        })
        .collect();

    let mut result_text = String::new();
    if filtered_speakers.is_empty() {
        result_text.push_str("No speakers found matching the criteria.");
    } else {
        for speaker in &filtered_speakers {
            result_text.push_str(&format!("Speaker: {}\n", speaker.name));
            result_text.push_str("Styles:\n");
            for style in &speaker.styles {
                result_text.push_str(&format!("  - {} (ID: {})\n", style.name, style.id));
            }
            result_text.push('\n');
        }
        result_text.push_str(&format!(
            "Total speakers found: {}",
            filtered_speakers.len()
        ));
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Text cannot be empty"));
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
        assert!(result.unwrap_err().to_string().contains("Text too long"));
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
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Rate must be between 0.5 and 2.0"));
    }

    #[tokio::test]
    async fn test_text_to_speech_invalid_style_id() {
        let args = json!({
            "text": "テスト",
            "style_id": 1001,
            "streaming": false
        });

        let result = handle_text_to_speech(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid style_id"));
    }
}
