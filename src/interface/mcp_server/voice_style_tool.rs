use anyhow::{Context, Result};
use serde_json::Value;

use crate::domain::voice_style::{
    filter_speakers, normalized_filters, render_voice_styles_result, ListVoiceStylesParams,
};
use crate::interface::cli::connect_daemon_rpc_auto_start;
use crate::interface::mcp_server::tool_types::text_result;

async fn connect_daemon_rpc_for_tool() -> Result<crate::interface::cli::DaemonRpcClient> {
    let socket_path = crate::infrastructure::paths::get_socket_path();
    connect_daemon_rpc_auto_start(&socket_path)
        .await
        .context("Failed to connect to VOICEVOX daemon")
}

/// Executes the `list_voice_styles` tool with optional speaker/style filters.
///
/// # Errors
///
/// Returns an error if parameters are invalid or the daemon cannot be contacted.
pub async fn handle_voice_style_list_tool(
    arguments: Value,
) -> Result<crate::interface::mcp_server::tool_types::ToolCallResult> {
    let params: ListVoiceStylesParams =
        serde_json::from_value(arguments).context("Invalid parameters for list_voice_styles")?;

    let mut client = connect_daemon_rpc_for_tool().await?;
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
