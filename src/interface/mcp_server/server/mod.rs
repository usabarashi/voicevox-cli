mod runtime;
mod stdio;

use anyhow::Result;

pub use stdio::run_stdio_server as run_mcp_server;

pub async fn run_tool_request(
    tool_name: &str,
    arguments: serde_json::Value,
    cancel_rx: Option<tokio::sync::oneshot::Receiver<String>>,
) -> Result<crate::interface::mcp_server::tools::types::ToolCallResult> {
    crate::interface::mcp_server::tools::registry::execute_tool_request(
        tool_name, arguments, cancel_rx,
    )
    .await
}
