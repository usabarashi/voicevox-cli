use anyhow::{Context, Result};
use rodio::Sink;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::app::{synthesize_bytes_via_daemon, DaemonSynthesisBytesRequest, NoopAppOutput};
use crate::client::format_daemon_rpc_error_for_mcp;
use crate::mcp::playback::{
    append_wav_segments_to_sink, play_daemon_audio_with_cancellation,
    wait_for_sink_with_cancellation, PlaybackOutcome,
};
use crate::mcp::tool_types::text_result;
use crate::mcp::tts_params::{parse_synthesize_params, text_char_count, SynthesizeParams};
use crate::synthesis::{
    prepare_backend_with_config, synthesize_streaming_segments, validate_basic_request,
    PreparedBackend, TextSynthesisRequest,
};

fn cancelled_message(reason: &str) -> String {
    if reason.is_empty() {
        "Audio playback cancelled by client".to_string()
    } else {
        format!("Audio playback cancelled: {reason}")
    }
}

fn streaming_success_message(text_len: usize, style_id: u32) -> String {
    format!(
        "Successfully synthesized {text_len} characters using style ID {style_id} in streaming mode"
    )
}

fn daemon_success_message(text_len: usize, style_id: u32, audio_size: usize) -> String {
    format!(
        "Successfully synthesized {text_len} characters using style ID {style_id} (audio size: {audio_size} bytes)"
    )
}

fn cancelled_result(reason: &str) -> crate::mcp::tool_types::ToolCallResult {
    text_result(cancelled_message(reason), true)
}

fn daemon_playback_result(
    outcome: PlaybackOutcome,
    text_len: usize,
    style_id: u32,
    audio_size: usize,
) -> crate::mcp::tool_types::ToolCallResult {
    match outcome {
        PlaybackOutcome::Completed => text_result(
            daemon_success_message(text_len, style_id, audio_size),
            false,
        ),
        PlaybackOutcome::Cancelled(reason) => cancelled_result(&reason),
    }
}

/// Executes the `text_to_speech` tool without external cancellation.
///
/// # Errors
///
/// Returns an error if parameter validation, synthesis, or playback setup fails.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech(
    arguments: Value,
) -> Result<crate::mcp::tool_types::ToolCallResult> {
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
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let params = parse_synthesize_params(arguments)?;
    validate_basic_request(&TextSynthesisRequest {
        text: &params.text,
        style_id: params.style_id,
        rate: params.rate,
    })?;

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
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;
    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;
    let sink = Arc::new(Sink::connect_new(stream.mixer()));

    let config = crate::config::Config::default();
    let mut synthesizer = match prepare_backend_with_config(true, &config).await {
        Ok(PreparedBackend::Streaming(synthesizer)) => synthesizer,
        Ok(PreparedBackend::Daemon(_)) => unreachable!(),
        Err(error) => return Err(error.context("Failed to create streaming synthesizer")),
    };

    let text_len = text_char_count(&text);

    let request = TextSynthesisRequest {
        text: &text,
        style_id,
        rate,
    };
    let wav_segments = synthesize_streaming_segments(&mut synthesizer, &request)
        .await
        .context("Streaming synthesis failed")?;
    append_wav_segments_to_sink(&sink, &wav_segments)?;

    let sink_clone = Arc::clone(&sink);
    let cancelled_reason = match wait_for_sink_with_cancellation(sink_clone, cancel_rx).await? {
        PlaybackOutcome::Completed => None,
        PlaybackOutcome::Cancelled(reason) => Some(reason),
    };

    if let Some(reason) = cancelled_reason {
        return Ok(cancelled_result(&reason));
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
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let socket_path = crate::paths::get_socket_path();
    let output = NoopAppOutput;
    let synth_request = DaemonSynthesisBytesRequest {
        text: &params.text,
        style_id: params.style_id,
        rate: params.rate,
        socket_path: &socket_path,
        ensure_models_if_missing: false,
        quiet_setup_messages: true,
    };
    let wav_data = match synthesize_bytes_via_daemon(&synth_request, &output).await {
        Ok(wav_data) => wav_data,
        Err(error) => {
            return Ok(text_result(format_daemon_rpc_error_for_mcp(&error), true));
        }
    };

    let audio_size = wav_data.len();
    let text_len = text_char_count(&params.text);
    let style_id = params.style_id;

    let outcome = play_daemon_audio_with_cancellation(wav_data, cancel_rx).await?;
    Ok(daemon_playback_result(
        outcome, text_len, style_id, audio_size,
    ))
}
