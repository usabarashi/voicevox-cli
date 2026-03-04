use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use super::types::{ToolCallResult, audio_result, text_result};
use crate::domain::synthesis::wav::concatenate_wav_segments;
use crate::domain::synthesis::{TextSynthesisRequest, validate_basic_request};
use crate::domain::text_to_speech::{
    SynthesizeParams, default_rate, default_streaming, text_char_count, validate_style_id,
};
use crate::infrastructure::daemon::startup;
use crate::interface::mcp_server::daemon_error::{
    format_daemon_client_error_for_mcp, is_retryable_daemon_synthesis_error,
};
use crate::interface::playback::{PlaybackOutcome, PlaybackRequest, emit_and_play};
use crate::interface::synthesis::flow::{
    DaemonSynthesisBytesRequest, NoopAppOutput, SynthesisFlowOutcome,
    synthesize_bytes_via_daemon_cancellable,
};
use crate::interface::synthesis::mode::{SynthesisMode, select_synthesis_mode_with_config};

const MCP_DAEMON_MAX_RETRIES: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpTtsPhase {
    Attempt,
    Backoff,
    Finish,
}

#[derive(Debug, Deserialize)]
struct TextToSpeechToolInput {
    text: String,
    style_id: u32,
    #[serde(default = "default_rate")]
    rate: f32,
    #[serde(default = "default_streaming")]
    streaming: bool,
}

enum DaemonRetryStep {
    Next(McpTtsPhase),
    Finish,
    Return(ToolCallResult),
}

struct DaemonRetryContext<'a> {
    text: &'a str,
    style_id: u32,
    rate: f32,
    socket_path: &'a std::path::Path,
    output: &'a NoopAppOutput,
    attempt: &'a mut u32,
    retry_delay: &'a mut Duration,
    last_error: &'a mut Option<anyhow::Error>,
    wav_data: &'a mut Option<Vec<u8>>,
    cancel_rx: &'a mut Option<oneshot::Receiver<String>>,
}

/// Executes the `text_to_speech` tool without external cancellation.
///
/// # Errors
///
/// Returns an error if parameter validation or synthesis fails.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech(arguments: Value) -> Result<ToolCallResult> {
    handle_text_to_speech_cancellable(arguments, None).await
}

/// Executes the `text_to_speech` tool with optional cancellation support.
///
/// Returns base64-encoded WAV audio data for client-side playback.
///
/// # Errors
///
/// Returns an error if parameters are invalid or synthesis fails.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech_cancellable(
    arguments: Value,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let parsed: TextToSpeechToolInput =
        serde_json::from_value(arguments).context("Invalid parameters for text_to_speech")?;
    validate_style_id(parsed.style_id)?;
    let params = SynthesizeParams {
        text: parsed.text,
        style_id: parsed.style_id,
        rate: parsed.rate,
        streaming: parsed.streaming,
    };
    validate_basic_request(&TextSynthesisRequest {
        text: &params.text,
        style_id: params.style_id,
        rate: params.rate,
    })?;

    if params.streaming {
        handle_streaming_synthesis(params, cancel_rx).await
    } else {
        handle_daemon_synthesis(params, cancel_rx).await
    }
}

/// Runs a potentially non-Send text-to-speech async task on a blocking worker thread.
pub fn spawn_non_send_text_to_speech_task<F>(future_factory: F)
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + Send + 'static,
{
    let runtime_handle = Handle::current();
    tokio::task::spawn_blocking(move || {
        runtime_handle.block_on(future_factory());
    });
}

#[allow(clippy::future_not_send)]
async fn handle_streaming_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;
    let text_len = text_char_count(&text);
    let synthesis = do_streaming_synthesis(&text, style_id, rate);

    if let Some(mut cancel_rx) = cancel_rx {
        if let Some(reason) = try_take_cancellation(&mut cancel_rx) {
            return Ok(cancellation_result(reason));
        }
        let wav_data = tokio::select! {
            result = synthesis => result,
            reason = &mut cancel_rx => {
                return Ok(cancellation_result(reason.unwrap_or_default()));
            }
        }?;
        if let Some(cancelled_result) = play_generated_audio(&wav_data, Some(cancel_rx)).await? {
            return Ok(cancelled_result);
        }
        Ok(audio_result(
            synthesis_success_message(text_len, style_id),
            &wav_data,
        ))
    } else {
        let wav_data = synthesis.await?;
        play_generated_audio(&wav_data, None).await?;
        Ok(audio_result(
            synthesis_success_message(text_len, style_id),
            &wav_data,
        ))
    }
}

#[allow(clippy::future_not_send)]
async fn do_streaming_synthesis(text: &str, style_id: u32, rate: f32) -> Result<Vec<u8>> {
    let config = crate::config::Config::default();
    let mut synthesizer = match select_synthesis_mode_with_config(true, &config).await {
        Ok(SynthesisMode::Streaming(synthesizer)) => synthesizer,
        Ok(SynthesisMode::Daemon(_)) => unreachable!(),
        Err(error) => return Err(error.context("Failed to create streaming synthesizer")),
    };

    let request = TextSynthesisRequest {
        text,
        style_id,
        rate,
    };
    let wav_segments = synthesizer
        .request_streaming_synthesis_segments(request.text, request.style_id, request.rate)
        .await
        .context("Streaming synthesis failed")?;

    let wav_data =
        concatenate_wav_segments(&wav_segments).context("Failed to concatenate WAV segments")?;

    Ok(wav_data)
}

#[allow(clippy::future_not_send)]
async fn handle_daemon_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;

    let socket_path = crate::infrastructure::paths::get_socket_path();
    let output = NoopAppOutput;
    let mut retry_delay = startup::initial_retry_delay();
    let mut last_error = None;
    let mut wav_data = None;
    let mut cancel_rx = cancel_rx;

    let mut attempt: u32 = 0;
    let mut phase = McpTtsPhase::Attempt;
    let mut ctx = DaemonRetryContext {
        text: &text,
        style_id,
        rate,
        socket_path: &socket_path,
        output: &output,
        attempt: &mut attempt,
        retry_delay: &mut retry_delay,
        last_error: &mut last_error,
        wav_data: &mut wav_data,
        cancel_rx: &mut cancel_rx,
    };

    loop {
        match run_daemon_retry_phase(phase, &mut ctx).await? {
            DaemonRetryStep::Next(next) => phase = next,
            DaemonRetryStep::Finish => break,
            DaemonRetryStep::Return(result) => return Ok(result),
        }
    }

    let Some(wav_data) = wav_data else {
        let error = last_error.expect("last error should exist when synthesis failed");
        return Ok(text_result(
            format_daemon_client_error_for_mcp(&error),
            true,
        ));
    };

    let text_len = text_char_count(&text);
    if let Some(cancelled_result) = play_generated_audio(&wav_data, cancel_rx).await? {
        return Ok(cancelled_result);
    }

    Ok(audio_result(
        synthesis_success_message(text_len, style_id),
        &wav_data,
    ))
}

#[allow(clippy::future_not_send)]
async fn run_daemon_retry_phase(
    phase: McpTtsPhase,
    ctx: &mut DaemonRetryContext<'_>,
) -> Result<DaemonRetryStep> {
    match phase {
        McpTtsPhase::Attempt => {
            if let Some(cancel_rx) = ctx.cancel_rx.as_mut()
                && let Some(reason) = try_take_cancellation(cancel_rx)
            {
                return Ok(DaemonRetryStep::Return(cancellation_result(reason)));
            }

            let synth_request = DaemonSynthesisBytesRequest {
                text: ctx.text,
                style_id: ctx.style_id,
                rate: ctx.rate,
                socket_path: ctx.socket_path,
                ensure_models_if_missing: false,
                quiet_setup_messages: true,
            };

            match synthesize_bytes_via_daemon_cancellable(
                &synth_request,
                ctx.output,
                ctx.cancel_rx.as_mut(),
            )
            .await
            {
                Ok(SynthesisFlowOutcome::Completed(result)) => {
                    *ctx.wav_data = Some(result);
                    Ok(DaemonRetryStep::Next(McpTtsPhase::Finish))
                }
                Ok(SynthesisFlowOutcome::Canceled(reason)) => {
                    Ok(DaemonRetryStep::Return(cancellation_result(reason)))
                }
                Err(error) => {
                    let retryable = is_retryable_daemon_synthesis_error(&error);
                    *ctx.last_error = Some(error);
                    if !retryable || *ctx.attempt >= MCP_DAEMON_MAX_RETRIES {
                        Ok(DaemonRetryStep::Next(McpTtsPhase::Finish))
                    } else {
                        Ok(DaemonRetryStep::Next(McpTtsPhase::Backoff))
                    }
                }
            }
        }
        McpTtsPhase::Backoff => {
            if let Some(cancel_rx) = ctx.cancel_rx.as_mut() {
                tokio::select! {
                    reason = cancel_rx => {
                        return Ok(DaemonRetryStep::Return(
                            cancellation_result(reason.unwrap_or_default())
                        ));
                    }
                    _ = tokio::time::sleep(*ctx.retry_delay) => {}
                }
            } else {
                tokio::time::sleep(*ctx.retry_delay).await;
            }
            *ctx.attempt += 1;
            *ctx.retry_delay = (*ctx.retry_delay * 2).min(startup::max_retry_delay());
            Ok(DaemonRetryStep::Next(McpTtsPhase::Attempt))
        }
        McpTtsPhase::Finish => Ok(DaemonRetryStep::Finish),
    }
}

fn synthesis_success_message(text_len: usize, style_id: u32) -> String {
    format!("Synthesized {text_len} characters using style ID {style_id}")
}

fn cancellation_message(reason: &str) -> String {
    if reason.is_empty() {
        "Synthesis cancelled".to_string()
    } else {
        format!("Synthesis cancelled: {reason}")
    }
}

fn cancellation_result(reason: String) -> ToolCallResult {
    text_result(cancellation_message(&reason), true)
}

fn try_take_cancellation(cancel_rx: &mut oneshot::Receiver<String>) -> Option<String> {
    match cancel_rx.try_recv() {
        Ok(reason) => Some(reason),
        Err(oneshot::error::TryRecvError::Closed) => Some(String::new()),
        Err(oneshot::error::TryRecvError::Empty) => None,
    }
}

#[allow(clippy::future_not_send)]
async fn play_generated_audio(
    wav_data: &[u8],
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<Option<ToolCallResult>> {
    match emit_and_play(PlaybackRequest {
        wav_data,
        output_file: None,
        play: true,
        cancel_rx,
    })
    .await
    .context("Failed to play synthesized audio")?
    {
        PlaybackOutcome::Completed => Ok(None),
        PlaybackOutcome::Cancelled(reason) => Ok(Some(cancellation_result(reason))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::daemon::client::daemon_response_error;
    use crate::infrastructure::ipc::DaemonErrorCode;
    use crate::interface::mcp_server::tools::types::ToolContent;
    use serde_json::json;
    use tokio::sync::oneshot;

    #[test]
    fn retryable_policy_matches_daemon_error_code() {
        let invalid = daemon_response_error("ctx", DaemonErrorCode::InvalidTargetId, "bad id");
        assert!(!is_retryable_daemon_synthesis_error(&invalid));

        let model_load =
            daemon_response_error("ctx", DaemonErrorCode::ModelLoadFailed, "model missing");
        assert!(!is_retryable_daemon_synthesis_error(&model_load));

        let synthesis =
            daemon_response_error("ctx", DaemonErrorCode::SynthesisFailed, "temporary failure");
        assert!(is_retryable_daemon_synthesis_error(&synthesis));

        let internal = daemon_response_error("ctx", DaemonErrorCode::Internal, "daemon panic");
        assert!(is_retryable_daemon_synthesis_error(&internal));
    }

    #[tokio::test]
    async fn cancellation_signal_short_circuits_daemon_synthesis() {
        let args = json!({
            "text": "テスト",
            "style_id": 3,
            "streaming": false,
        });
        let (cancel_tx, cancel_rx) = oneshot::channel::<String>();
        let _ = cancel_tx.send("ESC pressed".to_string());

        let result = handle_text_to_speech_cancellable(args, Some(cancel_rx))
            .await
            .expect("cancellation should return tool result");

        assert_eq!(result.is_error, Some(true));
        let Some(ToolContent::Text { text }) = result.content.first() else {
            panic!("expected text content in cancellation result");
        };
        assert!(text.contains("cancelled"));
        assert!(text.contains("ESC pressed"));
    }
}
