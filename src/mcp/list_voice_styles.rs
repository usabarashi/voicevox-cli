use anyhow::{Context, Result};
use serde_json::Value;

use crate::client::DaemonClient;
use crate::mcp::tool_types::text_result;
use crate::mcp::voice_style_query::{
    filter_speakers, normalized_filters, render_voice_styles_result, ListVoiceStylesParams,
};

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
pub async fn handle_list_voice_styles(
    arguments: Value,
) -> Result<crate::mcp::tool_types::ToolCallResult> {
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
