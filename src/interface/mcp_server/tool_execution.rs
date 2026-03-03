use anyhow::{anyhow, Result};
use serde_json::Value;
use tokio::sync::oneshot;

pub use crate::interface::mcp_server::speech_synthesis_tool::{
    handle_text_to_speech, handle_text_to_speech_cancellable,
};
pub use crate::interface::mcp_server::tool_catalog::{get_tool_definitions, ToolDefinition};
pub use crate::interface::mcp_server::tool_types::{ToolCallResult, ToolContent};
pub use crate::interface::mcp_server::voice_style_tool::handle_voice_style_list_tool;

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
        "list_voice_styles" => handle_voice_style_list_tool(arguments).await,
        _ => Err(anyhow!("Unknown tool: {tool_name}")),
    }
}
