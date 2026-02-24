use anyhow::{Context, Result};
use rodio::Sink;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::mcp::playback::{play_daemon_audio_with_cancellation, PlaybackOutcome};
use crate::mcp::tool_types::text_result;
use crate::mcp::tts_params::{parse_synthesize_params, text_char_count, SynthesizeParams};
use crate::synthesis::{prepare_backend, PreparedBackend};

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

    let mut synthesizer = match prepare_backend(true).await {
        Ok(PreparedBackend::Streaming(synthesizer)) => synthesizer,
        Ok(PreparedBackend::Daemon(_)) => unreachable!(),
        Err(error) => return Err(error.context("Failed to create streaming synthesizer")),
    };

    let sink_clone = Arc::clone(&sink);
    let text_len = text_char_count(&text);

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
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let mut client = match prepare_backend(false).await {
        Ok(PreparedBackend::Daemon(client)) => client,
        Ok(PreparedBackend::Streaming(_)) => unreachable!(),
        Err(error) => {
            return Ok(text_result(
                format!("Failed to connect to VOICEVOX daemon: {error}"),
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
    let text_len = text_char_count(&params.text);
    let style_id = params.style_id;

    let outcome = play_daemon_audio_with_cancellation(wav_data, cancel_rx).await?;
    Ok(daemon_playback_result(
        outcome, text_len, style_id, audio_size,
    ))
}
