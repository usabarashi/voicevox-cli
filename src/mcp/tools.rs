use anyhow::{anyhow, Context, Result};
use rodio::Sink;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::client::{audio::play_audio_from_memory, DaemonClient};
use crate::synthesis::StreamingSynthesizer;

// Tool Definition Types
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: ToolInputSchema,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: serde_json::Map<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

// Tool Execution Result Types
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(rename = "isError", skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "text_to_speech".to_string(),
            description: "Convert Japanese text to speech with VOICEVOX. Splits long messages automatically for client compatibility.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: json!({
                    "text": {
                        "type": "string",
                        "description": "Japanese text (15-50 chars optimal, 100+ may need splitting)"
                    },
                    "style_id": {
                        "type": "integer",
                        "description": "3=normal, 1=happy, 22=whisper, 76=sad, 75=confused"
                    },
                    "rate": {
                        "type": "number",
                        "description": "Speed (0.5-2.0, default 1.0)",
                        "minimum": 0.5,
                        "maximum": 2.0,
                        "default": 1.0
                    },
                    "streaming": {
                        "type": "boolean",
                        "description": "Lower latency mode",
                        "default": true
                    }
                })
                .as_object()
                .unwrap_or(&serde_json::Map::new())
                .clone(),
                required: Some(vec!["text".to_string(), "style_id".to_string()]),
            },
        },
        ToolDefinition {
            name: "list_voice_styles".to_string(),
            description: "Get available VOICEVOX voice styles for text_to_speech. Use this before synthesizing speech to discover available style_ids and their characteristics. Filter by speaker_name or style_name (e.g., 'ノーマル', 'ささやき', 'なみだめ') to find appropriate voices. Returns style_id, speaker name, and style type for each voice. Call this when users ask about available voices or when you need to select an appropriate voice style based on context.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: json!({
                    "speaker_name": {
                        "type": "string",
                        "description": "Filter by speaker name (partial match)"
                    },
                    "style_name": {
                        "type": "string",
                        "description": "Filter by style name (partial match)"
                    }
                })
                .as_object()
                .unwrap_or(&serde_json::Map::new())
                .clone(),
                required: None,
            },
        },
    ]
}

/// Executes an MCP tool request with cancellation support.
///
/// This is the main entry point for tool execution, dispatching requests to
/// the appropriate tool handler based on the tool name.
///
/// ## Supported Tools
///
/// - `text_to_speech`: Japanese text-to-speech synthesis with cancellation
/// - `list_voice_styles`: Voice style enumeration (no cancellation needed)
///
/// ## Parameters
///
/// - `tool_name`: Name of the tool to execute
/// - `arguments`: Tool execution arguments
/// - `cancel_rx`: Optional cancellation receiver channel
///
/// ## Returns
///
/// - `Ok(ToolCallResult)`: Successful tool execution result
/// - `Err(anyhow::Error)`: Tool execution error or unknown tool
pub async fn execute_tool_request(
    tool_name: &str,
    arguments: Value,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    match tool_name {
        "text_to_speech" => handle_text_to_speech_cancellable(arguments, cancel_rx).await,
        "list_voice_styles" => handle_list_voice_styles(arguments).await,
        _ => Err(anyhow!("Unknown tool: {}", tool_name)),
    }
}

const MAX_STYLE_ID: u32 = 1000;

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
    handle_text_to_speech_cancellable(arguments, None).await
}

pub async fn handle_text_to_speech_cancellable(
    arguments: Value,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
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

    (params.style_id <= MAX_STYLE_ID)
        .then_some(())
        .ok_or_else(|| {
            anyhow!(
                "Invalid style_id: {} (max: {})",
                params.style_id,
                MAX_STYLE_ID
            )
        })?;

    if params.streaming {
        handle_streaming_synthesis_cancellable(params, cancel_rx).await
    } else {
        handle_daemon_synthesis(params).await
    }
}

async fn handle_streaming_synthesis_cancellable(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;

    let sink = Arc::new(Sink::connect_new(stream.mixer()));

    let mut synthesizer = StreamingSynthesizer::new()
        .await
        .context("Failed to create streaming synthesizer")?;

    synthesizer
        .synthesize_streaming(&params.text, params.style_id, params.rate, &sink)
        .await
        .context("Streaming synthesis failed")?;

    // Cancellable playback
    if let Some(mut cancel_rx) = cancel_rx {
        let sink_clone = Arc::clone(&sink);
        tokio::select! {
            result = tokio::task::spawn_blocking(move || { sink_clone.sleep_until_end(); }) => {
                result.context("Audio playback task failed")?;
            }
            reason = &mut cancel_rx => {
                // Cancel received, stop audio by dropping sink
                drop(sink);
                drop(stream);
                return Ok(ToolCallResult {
                    content: vec![ToolContent {
                        content_type: "text".to_string(),
                        text: format!("Audio playback cancelled: {}", reason.unwrap_or_default()),
                    }],
                    is_error: Some(false),
                });
            }
        }
    } else {
        let sink_clone = Arc::clone(&sink);
        tokio::task::spawn_blocking(move || {
            sink_clone.sleep_until_end();
        })
        .await
        .context("Audio playback task failed")?;
    }

    drop(stream);

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

    let options = crate::ipc::OwnedSynthesizeOptions { rate: params.rate };

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
            "style_id": MAX_STYLE_ID + 1,
            "streaming": false
        });

        let result = handle_text_to_speech(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid style_id"));
    }
}
