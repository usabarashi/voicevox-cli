use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::mcp::protocol::{JsonRpcResponse, INTERNAL_ERROR};
use crate::mcp::tools::{self, ToolCallResult, ToolContent};

/// Manages active requests and their cancellation tokens.
///
/// This structure implements the server-side cancellation management for MCP requests.
/// It tracks active requests and provides a mechanism to cancel them through oneshot channels.
///
/// ## MCP Protocol Reference
///
/// Implements cancellation support as specified in:
/// <https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/cancellation>
///
/// ## Usage
///
/// 1. Register a request with `register()` when starting execution
/// 2. Client sends `notifications/cancelled` to cancel the request
/// 3. Call `cancel()` to send cancellation signal to the executing task
/// 4. Call `complete()` to clean up finished requests
#[derive(Debug, Clone)]
pub struct ActiveRequests {
    abort_channels: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
    response_sender: mpsc::UnboundedSender<JsonRpcResponse>,
}

impl ActiveRequests {
    pub fn new(response_sender: mpsc::UnboundedSender<JsonRpcResponse>) -> Self {
        Self {
            abort_channels: Arc::new(Mutex::new(HashMap::new())),
            response_sender,
        }
    }

    /// Register a new request with its cancellation channel.
    ///
    /// This should be called when starting execution of an MCP tool call.
    /// The `sender` will be used to deliver cancellation signals if the client
    /// sends a `notifications/cancelled` message for this request.
    ///
    /// ## Parameters
    ///
    /// - `request_id`: The unique identifier from the original MCP request
    /// - `sender`: The oneshot channel sender for delivering cancellation signals
    pub async fn register(&self, request_id: String, sender: oneshot::Sender<String>) {
        self.abort_channels.lock().await.insert(request_id, sender);
    }

    /// Cancel a request by sending the cancellation signal.
    ///
    /// This method is called when a `notifications/cancelled` message is received
    /// from the MCP client. It looks up the request by ID and sends the cancellation
    /// reason through the associated oneshot channel.
    ///
    /// ## Parameters
    ///
    /// - `request_id`: The ID of the request to cancel
    /// - `reason`: Optional human-readable cancellation reason
    ///
    /// ## Returns
    ///
    /// - `true` if the request was found and cancellation signal was sent
    /// - `false` if the request was not found (already completed or invalid ID)
    pub async fn cancel(&self, request_id: &str, reason: Option<String>) -> bool {
        if let Some(sender) = self.abort_channels.lock().await.remove(request_id) {
            let _ = sender.send(reason.unwrap_or_default());
            true
        } else {
            false
        }
    }

    /// Remove a completed request from the active list.
    ///
    /// This should be called when a request completes (either successfully,
    /// with an error, or due to cancellation) to clean up resources and
    /// prevent memory leaks.
    ///
    /// ## Parameters
    ///
    /// - `request_id`: The ID of the completed request
    pub async fn complete(&self, request_id: &str) {
        self.abort_channels.lock().await.remove(request_id);
    }

    /// Spawns an asynchronous execution for the requested MCP tool call with cancellation support.
    ///
    /// Creates a oneshot channel for cancellation signaling, registers the request
    /// with the active requests manager, and spawns a blocking task to execute the request.
    /// The execution automatically cleans up after completion and sends the response to stdout.
    ///
    /// ## MCP Protocol Reference
    ///
    /// Implements asynchronous request execution as specified in:
    /// <https://modelcontextprotocol.io/specification/2025-06-18/server/tools>
    ///
    /// ## Parameters
    ///
    /// - `request_id`: Unique identifier for request tracking and cancellation
    /// - `id`: JSON-RPC request ID for response correlation
    /// - `tool_name`: Name of the tool to execute
    /// - `arguments`: Tool execution arguments
    pub async fn spawn_execution(
        &self,
        request_id: String,
        id: Value,
        tool_name: &str,
        arguments: Value,
    ) {
        let (abort_tx, abort_rx) = oneshot::channel::<String>();

        // Register the cancellation channel
        self.register(request_id.clone(), abort_tx).await;

        let tool_name = tool_name.to_string();
        let active_requests = self.clone();

        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let result =
                    tools::execute_tool_request(&tool_name, arguments, Some(abort_rx)).await;

                // Clean up the request from active list
                active_requests.complete(&request_id).await;

                // Send response
                let response = match result {
                    Ok(tool_result) => match serde_json::to_value(tool_result) {
                        Ok(value) => JsonRpcResponse::success(id, value),
                        Err(_) => JsonRpcResponse::error(
                            id,
                            INTERNAL_ERROR,
                            "Failed to serialize response".to_string(),
                        ),
                    },
                    Err(e) => {
                        let error_result = ToolCallResult {
                            content: vec![ToolContent {
                                content_type: "text".to_string(),
                                text: format!("Tool execution error: {e}"),
                            }],
                            is_error: Some(true),
                        };
                        match serde_json::to_value(error_result) {
                            Ok(value) => JsonRpcResponse::success(id, value),
                            Err(_) => JsonRpcResponse::error(
                                id,
                                INTERNAL_ERROR,
                                "Failed to serialize error response".to_string(),
                            ),
                        }
                    }
                };

                // Send response via channel
                let _ = active_requests.response_sender.send(response);
            })
        });
    }
}
