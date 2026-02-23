use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use tokio::sync::oneshot;

use crate::client::DaemonClient;
use crate::mcp::tool_types::text_result;
use crate::mcp::voice_style_query::{
    filter_speakers, normalized_filters, render_voice_styles_result, ListVoiceStylesParams,
};

pub use crate::mcp::tool_catalog::{get_tool_definitions, ToolDefinition};
pub use crate::mcp::tool_types::{ToolCallResult, ToolContent};
pub use crate::mcp::tts_execute::{handle_text_to_speech, handle_text_to_speech_cancellable};

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

async fn connect_daemon_client_for_tool() -> Result<DaemonClient> {
    DaemonClient::connect_with_retry()
        .await
        .context("Failed to connect to VOICEVOX daemon after multiple attempts")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tts_params;
    use crate::mcp::tts_params::SynthesizeParams;
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
            "style_id": tts_params::MAX_STYLE_ID + 1,
            "streaming": false
        });

        assert_tts_error_contains(args, "Invalid style_id").await;
    }

    #[test]
    fn test_validate_synthesize_params_char_limit_uses_character_count() {
        let params = SynthesizeParams {
            text: "あ".repeat(tts_params::MAX_TEXT_LENGTH),
            style_id: 3,
            rate: 1.0,
            streaming: false,
        };

        let result = tts_params::validate_synthesize_params(&params);
        assert!(
            result.is_ok(),
            "expected char-limit boundary to pass: {result:?}"
        );
    }
}
