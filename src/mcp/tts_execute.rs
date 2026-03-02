use anyhow::{Context, Result};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::daemon::startup;
use crate::app::{synthesize_bytes_via_daemon, DaemonSynthesisBytesRequest, NoopAppOutput};
use crate::client::{find_daemon_rpc_error, format_daemon_rpc_error_for_mcp};
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

const MCP_DAEMON_MAX_RETRIES: u32 = 2;

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
    _cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let params = parse_synthesize_params(arguments)?;
    validate_basic_request(&TextSynthesisRequest {
        text: &params.text,
        style_id: params.style_id,
        rate: params.rate,
    })?;

    if params.streaming {
        handle_streaming_synthesis(params).await
    } else {
        handle_daemon_synthesis(params).await
    }
}

#[allow(clippy::future_not_send)]
async fn handle_streaming_synthesis(
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
) -> Result<crate::mcp::tool_types::ToolCallResult> {
    let socket_path = crate::paths::get_socket_path();
    let output = NoopAppOutput;
    let mut retry_delay = startup::initial_retry_delay();
    let mut last_error = None;
    let mut wav_data = None;

    for attempt in 0..=MCP_DAEMON_MAX_RETRIES {
        let synth_request = DaemonSynthesisBytesRequest {
            text: &params.text,
            style_id: params.style_id,
            rate: params.rate,
            socket_path: &socket_path,
            ensure_models_if_missing: false,
            quiet_setup_messages: true,
        };

        match synthesize_bytes_via_daemon(&synth_request, &output).await {
            Ok(result) => {
                wav_data = Some(result);
                break;
            }
            Err(error) => {
                let retryable = is_retryable_daemon_synthesis_error(&error);
                last_error = Some(error);
                if !retryable || attempt >= MCP_DAEMON_MAX_RETRIES {
                    break;
                }
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(startup::max_retry_delay());
            }
        }
    }

    let Some(wav_data) = wav_data else {
        let error = last_error.expect("last error should exist when synthesis failed");
        return Ok(text_result(format_daemon_rpc_error_for_mcp(&error), true));
    };

    let text_len = text_char_count(&params.text);

    Ok(audio_result(
        synthesis_success_message(text_len, params.style_id),
        &wav_data,
    ))
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
}
