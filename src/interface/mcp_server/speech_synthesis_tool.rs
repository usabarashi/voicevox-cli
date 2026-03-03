use anyhow::{Context, Result};
use serde_json::Value;
use std::time::Duration;
use tokio::sync::oneshot;

use crate::domain::synthesis::wav::concatenate_wav_segments;
use crate::domain::synthesis::{validate_basic_request, TextSynthesisRequest};
use crate::domain::text_to_speech::{
    parse_synthesize_params, text_char_count, McpTtsPhase, SynthesizeParams,
};
use crate::infrastructure::daemon::startup;
use crate::interface::cli::{
    emit_and_play, format_daemon_rpc_error_for_mcp, infer_voice_target_state,
    request_streaming_synthesis_segments, select_synthesis_mode_with_config, PlaybackOutcome,
    PlaybackRequest, SynthesisMode, VoiceTargetState,
};
use crate::interface::cli::{
    synthesize_bytes_via_daemon, DaemonSynthesisBytesRequest, NoopAppOutput,
};
use crate::interface::mcp_server::speech_synthesis_messages::{
    cancellation_result, synthesis_success_message, try_take_cancellation,
};
use crate::interface::mcp_server::tool_types::{audio_result, text_result};

const MCP_DAEMON_MAX_RETRIES: u32 = 2;

enum DaemonRetryStep {
    Next(McpTtsPhase),
    Finish,
    Return(crate::interface::mcp_server::tool_types::ToolCallResult),
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
pub async fn handle_text_to_speech(
    arguments: Value,
) -> Result<crate::interface::mcp_server::tool_types::ToolCallResult> {
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
) -> Result<crate::interface::mcp_server::tool_types::ToolCallResult> {
    let params = parse_synthesize_params(arguments)?;
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

#[allow(clippy::future_not_send)]
async fn handle_streaming_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<crate::interface::mcp_server::tool_types::ToolCallResult> {
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
    let wav_segments = request_streaming_synthesis_segments(&mut synthesizer, &request)
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
) -> Result<crate::interface::mcp_server::tool_types::ToolCallResult> {
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
        return Ok(text_result(format_daemon_rpc_error_for_mcp(&error), true));
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
            if let Some(cancel_rx) = ctx.cancel_rx.as_mut() {
                if let Some(reason) = try_take_cancellation(cancel_rx) {
                    return Ok(DaemonRetryStep::Return(cancellation_result(reason)));
                }
            }

            let synth_request = DaemonSynthesisBytesRequest {
                text: ctx.text,
                style_id: ctx.style_id,
                rate: ctx.rate,
                socket_path: ctx.socket_path,
                ensure_models_if_missing: false,
                quiet_setup_messages: true,
            };

            let synth_result = if let Some(cancel_rx) = ctx.cancel_rx.as_mut() {
                tokio::select! {
                    reason = cancel_rx => {
                        return Ok(DaemonRetryStep::Return(
                            cancellation_result(reason.unwrap_or_default())
                        ));
                    }
                    result = synthesize_bytes_via_daemon(&synth_request, ctx.output) => result,
                }
            } else {
                synthesize_bytes_via_daemon(&synth_request, ctx.output).await
            };

            match synth_result {
                Ok(result) => {
                    *ctx.wav_data = Some(result);
                    Ok(DaemonRetryStep::Next(McpTtsPhase::Finish))
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

fn is_retryable_daemon_synthesis_error(error: &anyhow::Error) -> bool {
    !matches!(infer_voice_target_state(error), VoiceTargetState::Missing)
}

#[allow(clippy::future_not_send)]
async fn play_generated_audio(
    wav_data: &[u8],
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<Option<crate::interface::mcp_server::tool_types::ToolCallResult>> {
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
    use crate::interface::cli::daemon_rpc::daemon_response_error;
    use crate::interface::ipc::DaemonErrorCode;
    use crate::interface::mcp_server::tool_types::ToolContent;
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
