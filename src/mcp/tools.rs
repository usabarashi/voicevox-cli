use anyhow::{anyhow, Context, Result};
use rodio::Sink;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt::Write as _;
use std::{env, path::Path, sync::Arc};
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

fn text_content(text: impl Into<String>) -> ToolContent {
    ToolContent {
        content_type: "text".to_string(),
        text: text.into(),
    }
}

fn text_result(text: impl Into<String>, is_error: bool) -> ToolCallResult {
    ToolCallResult {
        content: vec![text_content(text)],
        is_error: Some(is_error),
    }
}

fn json_object(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => serde_json::Map::new(),
    }
}

#[must_use]
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "text_to_speech".to_string(),
            description: "Convert Japanese text to speech with VOICEVOX. Splits long messages automatically for client compatibility.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: json_object(json!({
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
                })),
                required: Some(vec!["text".to_string(), "style_id".to_string()]),
            },
        },
        ToolDefinition {
            name: "list_voice_styles".to_string(),
            description: "Get available VOICEVOX voice styles for text_to_speech. Use this before synthesizing speech to discover available style_ids and their characteristics. Filter by speaker_name or style_name (e.g., 'ノーマル', 'ささやき', 'なみだめ') to find appropriate voices. Returns style_id, speaker name, and style type for each voice. Call this when users ask about available voices or when you need to select an appropriate voice style based on context.".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: json_object(json!({
                    "speaker_name": {
                        "type": "string",
                        "description": "Filter by speaker name (partial match)"
                    },
                    "style_name": {
                        "type": "string",
                        "description": "Filter by style name (partial match)"
                    }
                })),
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
///
/// # Errors
///
/// Returns an error if request dispatch fails or a tool handler returns an error.
#[allow(clippy::future_not_send)]
pub async fn execute_tool_request(
    tool_name: &str,
    arguments: Value,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    match tool_name {
        "text_to_speech" => handle_text_to_speech_cancellable(arguments, cancel_rx).await,
        "list_voice_styles" => handle_list_voice_styles(arguments).await,
        _ => Err(anyhow!("Unknown tool: {tool_name}")),
    }
}

const MAX_STYLE_ID: u32 = 1000;
const MAX_TEXT_LENGTH: usize = 10_000;

#[derive(Debug, Deserialize)]
struct SynthesizeParams {
    text: String,
    style_id: u32,
    #[serde(default = "default_rate")]
    rate: f32,
    #[serde(default = "default_streaming")]
    streaming: bool,
}

const fn default_rate() -> f32 {
    1.0
}

const fn default_streaming() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct ListVoiceStylesParams {
    speaker_name: Option<String>,
    style_name: Option<String>,
}

type FilteredSpeakerStyles<Name = String> = (Name, Vec<crate::voice::Style>);

fn cancelled_message(reason: &str) -> String {
    if reason.is_empty() {
        "Audio playback cancelled by client".to_string()
    } else {
        format!("Audio playback cancelled: {reason}")
    }
}

fn normalized_filters(params: &ListVoiceStylesParams) -> (Option<String>, Option<String>) {
    (
        params.speaker_name.as_ref().map(|s| s.to_lowercase()),
        params.style_name.as_ref().map(|s| s.to_lowercase()),
    )
}

fn filter_speakers(
    speakers: Vec<crate::voice::Speaker>,
    speaker_name_filter: Option<&str>,
    style_name_filter: Option<&str>,
) -> Vec<FilteredSpeakerStyles> {
    speakers
        .into_iter()
        .filter_map(|speaker| {
            let crate::voice::Speaker { name, styles, .. } = speaker;

            if let Some(name_filter) = speaker_name_filter {
                if !name.to_lowercase().contains(name_filter) {
                    return None;
                }
            }

            let filtered_styles = styles
                .into_iter()
                .filter(|style| {
                    style_name_filter
                        .is_none_or(|style_filter| style.name.to_lowercase().contains(style_filter))
                })
                .collect::<Vec<_>>();

            (!filtered_styles.is_empty()).then_some((name.to_string(), filtered_styles))
        })
        .collect()
}

fn streaming_success_message(text_len: usize, style_id: u32) -> String {
    format!("Successfully synthesized {text_len} characters using style ID {style_id} in streaming mode")
}

fn daemon_success_message(text_len: usize, style_id: u32, audio_size: usize) -> String {
    format!(
        "Successfully synthesized {text_len} characters using style ID {style_id} (audio size: {audio_size} bytes)"
    )
}

fn cancelled_result(reason: &str) -> ToolCallResult {
    text_result(cancelled_message(reason), true)
}

fn daemon_playback_result(
    outcome: PlaybackOutcome,
    text_len: usize,
    style_id: u32,
    audio_size: usize,
) -> ToolCallResult {
    match outcome {
        PlaybackOutcome::Completed => text_result(
            daemon_success_message(text_len, style_id, audio_size),
            false,
        ),
        PlaybackOutcome::Cancelled(reason) => cancelled_result(&reason),
    }
}

async fn connect_daemon_client_for_tool() -> Result<DaemonClient> {
    DaemonClient::connect_with_retry()
        .await
        .context("Failed to connect to VOICEVOX daemon after multiple attempts")
}

fn validate_synthesize_params(params: &SynthesizeParams) -> Result<()> {
    let text = params.text.trim();
    (!text.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!("Text cannot be empty"))?;

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

    Ok(())
}

fn parse_synthesize_params(arguments: Value) -> Result<SynthesizeParams> {
    let params: SynthesizeParams =
        serde_json::from_value(arguments).context("Invalid parameters for text_to_speech")?;
    validate_synthesize_params(&params)?;
    Ok(params)
}

/// Executes the `text_to_speech` tool without external cancellation.
///
/// # Errors
///
/// Returns an error if parameter validation, synthesis, or playback setup fails.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech(arguments: Value) -> Result<ToolCallResult> {
    handle_text_to_speech_cancellable(arguments, None).await
}

/// Executes the `text_to_speech` tool with optional cancellation support.
///
/// # Errors
///
/// Returns an error if parameters are invalid, synthesis fails, playback fails, or
/// daemon communication fails in non-streaming mode.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech_cancellable(
    arguments: Value,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let params = parse_synthesize_params(arguments)?;

    if params.streaming {
        handle_streaming_synthesis_cancellable(params, cancel_rx).await
    } else {
        handle_daemon_synthesis(params, cancel_rx).await
    }
}

#[allow(clippy::future_not_send)]
async fn handle_streaming_synthesis_cancellable(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;
    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;
    let sink = Arc::new(Sink::connect_new(stream.mixer()));

    let mut synthesizer = StreamingSynthesizer::new()
        .await
        .context("Failed to create streaming synthesizer")?;

    let sink_clone = Arc::clone(&sink);
    let text_len = text.len();

    let synthesis_and_playback_fut = async move {
        synthesizer
            .synthesize_streaming(&text, style_id, rate, &sink_clone)
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
                let reason = reason.unwrap_or_default();
                return Ok(cancelled_result(&reason));
            }
        }
    } else {
        synthesis_and_playback_fut.await?;
    }

    Ok(text_result(
        streaming_success_message(text_len, style_id),
        false,
    ))
}

#[allow(clippy::future_not_send)]
async fn handle_daemon_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let mut client = match connect_daemon_client_for_tool().await {
        Ok(client) => client,
        Err(e) => {
            return Ok(text_result(
                format!("Failed to connect to VOICEVOX daemon: {e}"),
                true,
            ));
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

    let outcome = play_daemon_audio_with_cancellation(wav_data, cancel_rx).await?;
    Ok(daemon_playback_result(
        outcome, text_len, style_id, audio_size,
    ))
}

enum PlaybackOutcome {
    Completed,
    Cancelled(String),
}

#[allow(clippy::future_not_send)]
async fn play_daemon_audio_with_cancellation(
    wav_data: Vec<u8>,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<PlaybackOutcome> {
    if let Some(mut cancel_rx) = cancel_rx {
        if env::var("VOICEVOX_LOW_LATENCY").is_ok() {
            play_low_latency_with_cancel(wav_data, &mut cancel_rx).await
        } else {
            play_system_player_with_cancel(&wav_data, &mut cancel_rx).await
        }
    } else {
        play_audio_from_memory(&wav_data).context("Failed to play audio")?;
        Ok(PlaybackOutcome::Completed)
    }
}

#[allow(clippy::future_not_send)]
async fn play_low_latency_with_cancel(
    wav_data: Vec<u8>,
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<PlaybackOutcome> {
    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;
    let sink = Arc::new(Sink::connect_new(stream.mixer()));
    let _stream_guard = stream;

    let cursor = std::io::Cursor::new(wav_data);
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

    let mut last_error = None;

    for command in ["afplay", "play"] {
        match run_player_with_cancel(command, &temp_path, cancel_rx).await {
            Ok(Some(outcome)) => return Ok(outcome),
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error
        .unwrap_or_else(|| anyhow!("No audio player found. Install sox or use -o to save file")))
}

async fn run_player_with_cancel(
    command: &str,
    temp_path: &Path,
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<Option<PlaybackOutcome>> {
    let mut child = match tokio::process::Command::new(command).arg(temp_path).spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("Failed to spawn {command}")),
    };

    tokio::select! {
        status = child.wait() => {
            let status = status.with_context(|| format!("Failed to wait for {command}"))?;
            if status.success() {
                Ok(Some(PlaybackOutcome::Completed))
            } else {
                Err(anyhow!(
                    "{command} exited with status {}",
                    status
                        .code()
                        .map_or_else(|| "terminated by signal".to_string(), |code| code.to_string())
                ))
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

/// Executes the `list_voice_styles` tool with optional speaker/style filters.
///
/// # Errors
///
/// Returns an error if parameters are invalid or the daemon cannot be contacted.
pub async fn handle_list_voice_styles(arguments: Value) -> Result<ToolCallResult> {
    let params: ListVoiceStylesParams =
        serde_json::from_value(arguments).context("Invalid parameters for list_voice_styles")?;

    let mut client = connect_daemon_client_for_tool().await?;

    let speakers = client.list_speakers().await?;

    let (speaker_name_filter, style_name_filter) = normalized_filters(&params);
    let filtered_results = filter_speakers(
        speakers,
        speaker_name_filter.as_deref(),
        style_name_filter.as_deref(),
    );

    let result_text = render_voice_styles_result(&filtered_results);
    Ok(text_result(result_text, false))
}

fn render_voice_styles_result<Name>(filtered_results: &[(Name, Vec<crate::voice::Style>)]) -> String
where
    Name: std::fmt::Display,
{
    if filtered_results.is_empty() {
        return "No speakers found matching the criteria.".to_string();
    }

    let mut result_text = filtered_results
        .iter()
        .map(render_voice_styles_block)
        .collect::<Vec<_>>()
        .join("\n");
    let _ = writeln!(result_text);
    let _ = write!(
        result_text,
        "Total speakers found: {}",
        filtered_results.len()
    );
    result_text
}

fn render_voice_styles_block<Name>(
    (speaker_name, styles): &(Name, Vec<crate::voice::Style>),
) -> String
where
    Name: std::fmt::Display,
{
    let mut block = format!("Speaker: {speaker_name}\nStyles:\n");
    for style in styles {
        let _ = writeln!(block, "  - {} (ID: {})", style.name, style.id);
    }
    block.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[allow(clippy::future_not_send)]
    async fn assert_tts_error_contains(args: Value, expected: &str) {
        let error_text = match handle_text_to_speech(args).await {
            Ok(result) => panic!("expected error, got success: {result:?}"),
            Err(error) => error.to_string(),
        };

        assert!(
            error_text.contains(expected),
            "expected error containing '{expected}', got '{error_text}'"
        );
    }

    #[tokio::test]
    async fn test_text_to_speech_empty_text() {
        let args = json!({
            "text": "",
            "style_id": 3,
            "streaming": false
        });

        assert_tts_error_contains(args, "Text cannot be empty").await;
    }

    #[tokio::test]
    async fn test_text_to_speech_text_too_long() {
        let long_text = "あ".repeat(10_001);
        let args = json!({
            "text": long_text,
            "style_id": 3,
            "streaming": false
        });

        assert_tts_error_contains(args, "Text too long").await;
    }

    #[tokio::test]
    async fn test_text_to_speech_invalid_rate() {
        let args = json!({
            "text": "テスト",
            "style_id": 3,
            "rate": 3.0,
            "streaming": false
        });

        assert_tts_error_contains(args, "Rate must be between 0.5 and 2.0").await;
    }

    #[tokio::test]
    async fn test_text_to_speech_invalid_style_id() {
        let args = json!({
            "text": "テスト",
            "style_id": MAX_STYLE_ID + 1,
            "streaming": false
        });

        assert_tts_error_contains(args, "Invalid style_id").await;
    }
}
