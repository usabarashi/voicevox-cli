use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;
use tokio::sync::oneshot;

use crate::app::{
    system_state::McpTtsPhase, synthesize_bytes_via_daemon, DaemonSynthesisBytesRequest,
    NoopAppOutput,
};
use crate::client::{find_daemon_rpc_error, format_daemon_rpc_error_for_mcp};
use crate::daemon::startup;
use crate::ipc::DaemonErrorCode;
use crate::mcp::tool_types::{audio_result, text_result};
use crate::mcp::tts_params::{parse_synthesize_params, text_char_count, SynthesizeParams};
use crate::synthesis::wav::concatenate_wav_segments;
use crate::synthesis::{
    prepare_backend_with_config, synthesize_streaming_segments, validate_basic_request,
    PreparedBackend, TextSynthesisRequest,
};

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

fn cancellation_result(reason: String) -> crate::mcp::tool_types::ToolCallResult {
    text_result(cancellation_message(&reason), true)
}

fn try_take_cancellation(cancel_rx: &mut oneshot::Receiver<String>) -> Option<String> {
    match cancel_rx.try_recv() {
        Ok(reason) => Some(reason),
        Err(oneshot::error::TryRecvError::Closed) => Some(String::new()),
        Err(oneshot::error::TryRecvError::Empty) => None,
    }
}

const MCP_DAEMON_MAX_RETRIES: u32 = 2;

enum DaemonSynthesisStep {
    Next(McpTtsPhase),
    Finish,
    Return(crate::mcp::tool_types::ToolCallResult),
}

/// Executes the `text_to_speech` tool without external cancellation.
///
/// # Errors
///
/// Returns an error if parameter validation or synthesis fails.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech(
    arguments: Value,
) -> Result<crate::mcp::tool_types::ToolCallResult> {
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
) -> Result<crate::mcp::tool_types::ToolCallResult> {
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
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let synthesis = do_streaming_synthesis(params);
    if let Some(mut cancel_rx) = cancel_rx {
        if let Some(reason) = try_take_cancellation(&mut cancel_rx) {
            return Ok(cancellation_result(reason));
        }
        tokio::select! {
            result = synthesis => result,
            reason = &mut cancel_rx => {
                Ok(cancellation_result(reason.unwrap_or_default()))
            }
        }
    } else {
        synthesis.await
    }
}

#[allow(clippy::future_not_send)]
async fn do_streaming_synthesis(
    params: SynthesizeParams,
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;

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

    let wav_data =
        concatenate_wav_segments(&wav_segments).context("Failed to concatenate WAV segments")?;

    Ok(audio_result(
        synthesis_success_message(text_len, style_id),
        &wav_data,
    ))
}

#[allow(clippy::future_not_send)]
async fn handle_daemon_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;

    let socket_path = crate::paths::get_socket_path();
    let output = NoopAppOutput;
    let mut retry_delay = startup::initial_retry_delay();
    let mut last_error = None;
    let mut wav_data = None;
    let mut cancel_rx = cancel_rx;

    let mut attempt: u32 = 0;
    let mut phase = McpTtsPhase::Attempt;

    loop {
        match run_daemon_synthesis_phase(
            phase,
            &text,
            style_id,
            rate,
            &socket_path,
            &output,
            &mut attempt,
            &mut retry_delay,
            &mut last_error,
            &mut wav_data,
            &mut cancel_rx,
        )
        .await?
        {
            DaemonSynthesisStep::Next(next) => phase = next,
            DaemonSynthesisStep::Finish => break,
            DaemonSynthesisStep::Return(result) => return Ok(result),
        }
    }

    let Some(wav_data) = wav_data else {
        let error = last_error.expect("last error should exist when synthesis failed");
        return Ok(text_result(format_daemon_rpc_error_for_mcp(&error), true));
    };

    let text_len = text_char_count(&text);

    Ok(audio_result(
        synthesis_success_message(text_len, style_id),
        &wav_data,
    ))
}

#[allow(clippy::future_not_send)]
async fn run_daemon_synthesis_phase(
    phase: McpTtsPhase,
    text: &str,
    style_id: u32,
    rate: f32,
    socket_path: &Path,
    output: &NoopAppOutput,
    attempt: &mut u32,
    retry_delay: &mut Duration,
    last_error: &mut Option<anyhow::Error>,
    wav_data: &mut Option<Vec<u8>>,
    cancel_rx: &mut Option<oneshot::Receiver<String>>,
) -> Result<DaemonSynthesisStep> {
    match phase {
        McpTtsPhase::Attempt => {
            if let Some(cancel_rx) = cancel_rx.as_mut() {
                if let Some(reason) = try_take_cancellation(cancel_rx) {
                    return Ok(DaemonSynthesisStep::Return(cancellation_result(reason)));
                }
            }

            let synth_request = DaemonSynthesisBytesRequest {
                text,
                style_id,
                rate,
                socket_path,
                ensure_models_if_missing: false,
                quiet_setup_messages: true,
            };

            let synth_result = if let Some(cancel_rx) = cancel_rx.as_mut() {
                tokio::select! {
                    reason = cancel_rx => {
                        return Ok(DaemonSynthesisStep::Return(
                            cancellation_result(reason.unwrap_or_default())
                        ));
                    }
                    result = synthesize_bytes_via_daemon(&synth_request, output) => result,
                }
            } else {
                synthesize_bytes_via_daemon(&synth_request, output).await
            };

            match synth_result {
                Ok(result) => {
                    *wav_data = Some(result);
                    Ok(DaemonSynthesisStep::Next(McpTtsPhase::Finish))
                }
                Err(error) => {
                    let retryable = is_retryable_daemon_synthesis_error(&error);
                    *last_error = Some(error);
                    if !retryable || *attempt >= MCP_DAEMON_MAX_RETRIES {
                        Ok(DaemonSynthesisStep::Next(McpTtsPhase::Finish))
                    } else {
                        Ok(DaemonSynthesisStep::Next(McpTtsPhase::Backoff))
                    }
                }
            }
        }
        McpTtsPhase::Backoff => {
            if let Some(cancel_rx) = cancel_rx.as_mut() {
                tokio::select! {
                    reason = cancel_rx => {
                        return Ok(DaemonSynthesisStep::Return(
                            cancellation_result(reason.unwrap_or_default())
                        ));
                    }
                    _ = tokio::time::sleep(*retry_delay) => {}
                }
            } else {
                tokio::time::sleep(*retry_delay).await;
            }
            *attempt += 1;
            *retry_delay = (*retry_delay * 2).min(startup::max_retry_delay());
            Ok(DaemonSynthesisStep::Next(McpTtsPhase::Attempt))
        }
        McpTtsPhase::Finish => Ok(DaemonSynthesisStep::Finish),
    }
}

fn is_retryable_daemon_synthesis_error(error: &anyhow::Error) -> bool {
    match find_daemon_rpc_error(error).map(|err| err.code()) {
        Some(DaemonErrorCode::InvalidTargetId) | Some(DaemonErrorCode::ModelLoadFailed) => false,
        Some(DaemonErrorCode::SynthesisFailed) | Some(DaemonErrorCode::Internal) => true,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::daemon_client::daemon_response_error;
    use crate::mcp::tool_types::ToolContent;
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
