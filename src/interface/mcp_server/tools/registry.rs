use anyhow::Result;
use serde_json::Value;
use tokio::sync::oneshot;

use super::list::{ToolDefinition, get_tool_definitions};
use super::types::ToolCallResult;

#[must_use]
pub fn tool_definitions() -> Vec<ToolDefinition> {
    get_tool_definitions()
}

#[allow(clippy::future_not_send)]
pub async fn execute_tool_request(
    tool_name: &str,
    arguments: Value,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    match tool_name {
        "text_to_speech" => {
            super::text_to_speech::handle_text_to_speech_cancellable(arguments, cancel_rx).await
        }
        "list_voice_styles" => {
            super::list_voice_styles::handle_voice_style_list_tool(arguments).await
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

pub async fn execute_send_tool_request(
    tool_name: &str,
    arguments: Value,
) -> Result<ToolCallResult> {
    match tool_name {
        "list_voice_styles" => {
            super::list_voice_styles::handle_voice_style_list_tool(arguments).await
        }
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}
