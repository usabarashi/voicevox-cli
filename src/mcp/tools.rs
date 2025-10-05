use anyhow::{anyhow, Context, Result};
use rodio::Sink;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{path::Path, sync::Arc};
use tokio::sync::oneshot;

use crate::client::{
    audio::{create_temp_wav_file, play_audio_from_memory},
    DaemonClient,
};
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
        handle_daemon_synthesis(params, cancel_rx).await
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

    let text = params.text.clone();
    let sink_clone = Arc::clone(&sink);

    let synthesis_and_playback_fut = async move {
        synthesizer
            .synthesize_streaming(&text, params.style_id, params.rate, &sink_clone)
            .await
            .context("Streaming synthesis failed")?;

        let res: Result<(), tokio::task::JoinError> = tokio::task::spawn_blocking(move || {
            sink_clone.sleep_until_end();
        })
        .await;
        res.context("Audio playback task failed")?;
        Ok(()) as Result<()>
    };

    if let Some(mut cancel_rx) = cancel_rx {
        tokio::pin!(synthesis_and_playback_fut);
        tokio::select! {
            res = &mut synthesis_and_playback_fut => {
                res?;
            }
            reason = &mut cancel_rx => {
                sink.stop();
                let detail = reason.unwrap_or_default();
                let message = if detail.is_empty() {
                    "Audio playback cancelled by client".to_string()
                } else {
                    format!("Audio playback cancelled: {detail}")
                };
                return Ok(ToolCallResult {
                    content: vec![ToolContent {
                        content_type: "text".to_string(),
                        text: message,
                    }],
                    is_error: Some(true),
                });
            }
        }
    } else {
        synthesis_and_playback_fut.await?;
    }

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

async fn handle_daemon_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
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

    let audio_size = wav_data.len();
    let text_len = params.text.len();
    let style_id = params.style_id;

    match play_daemon_audio_with_cancellation(wav_data, cancel_rx).await? {
        PlaybackOutcome::Completed => Ok(ToolCallResult {
            content: vec![ToolContent {
                content_type: "text".to_string(),
                text: format!(
                    "Successfully synthesized {text_len} characters using style ID {style_id} (audio size: {audio_size} bytes)"
                ),
            }],
            is_error: Some(false),
        }),
        PlaybackOutcome::Cancelled(reason) => {
            let message = if reason.is_empty() {
                "Audio playback cancelled by client".to_string()
            } else {
                format!("Audio playback cancelled: {reason}")
            };

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: message,
                }],
                is_error: Some(true),
            })
        }
    }
}

enum PlaybackOutcome {
    Completed,
    Cancelled(String),
}

async fn play_daemon_audio_with_cancellation(
    wav_data: Vec<u8>,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<PlaybackOutcome> {
    let shared_audio: Arc<[u8]> = wav_data.into();

    if let Some(mut cancel_rx) = cancel_rx {
        match play_low_latency_with_cancel(Arc::clone(&shared_audio), &mut cancel_rx).await {
            Ok(outcome) => Ok(outcome),
            Err(rodio_err) => play_system_player_with_cancel(shared_audio.as_ref(), &mut cancel_rx)
                .await
                .map_err(|system_err| {
                    system_err.context(format!("Low-latency audio playback failed: {rodio_err}"))
                }),
        }
    } else {
        play_audio_from_memory(shared_audio.as_ref()).context("Failed to play audio")?;
        Ok(PlaybackOutcome::Completed)
    }
}

async fn play_low_latency_with_cancel(
    wav_data: Arc<[u8]>,
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<PlaybackOutcome> {
    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;
    let sink = Arc::new(Sink::connect_new(stream.mixer()));
    let _stream_guard = stream;

    let cursor = std::io::Cursor::new(Arc::clone(&wav_data));
    let source = rodio::Decoder::new(cursor).context("Failed to decode audio")?;
    sink.append(source);
    sink.play();

    let playback_task = tokio::task::spawn_blocking({
        let sink_for_task = Arc::clone(&sink);
        move || -> Result<()> {
            sink_for_task.sleep_until_end();
            Ok(())
        }
    });
    tokio::pin!(playback_task);

    tokio::select! {
        res = &mut playback_task => {
            res.context("Audio playback task failed")??;
            Ok(PlaybackOutcome::Completed)
        }
        reason = cancel_rx => {
            let reason = reason.unwrap_or_default();
            sink.stop();
            let _ = playback_task.await;
            Ok(PlaybackOutcome::Cancelled(reason))
        }
    }
}

async fn play_system_player_with_cancel(
    wav_data: &[u8],
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<PlaybackOutcome> {
    // Hold the temp file open so external players can read it.
    let temp_file = create_temp_wav_file(wav_data)?;
    let temp_path = temp_file.path().to_owned();

    if let Some(outcome) = run_player_with_cancel("afplay", &temp_path, cancel_rx).await? {
        return Ok(outcome);
    }

    if let Some(outcome) = run_player_with_cancel("play", &temp_path, cancel_rx).await? {
        return Ok(outcome);
    }

    Err(anyhow!(
        "No audio player found. Install sox or use -o to save file"
    ))
}
async fn run_player_with_cancel(
    command: &str,
    temp_path: &Path,
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<Option<PlaybackOutcome>> {
    let mut child = match tokio::process::Command::new(command).arg(temp_path).spawn() {
        Ok(child) => child,
        Err(_) => return Ok(None),
    };

    tokio::select! {
        status = child.wait() => {
            let status = status.with_context(|| format!("Failed to wait for {command}"))?;
            if status.success() {
                Ok(Some(PlaybackOutcome::Completed))
            } else {
                Ok(None)
            }
        }
        reason = cancel_rx => {
            let reason = reason.unwrap_or_default();
            let _ = child.kill().await;
            let _ = child.wait().await;
            Ok(Some(PlaybackOutcome::Cancelled(reason)))
        }
    }
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
