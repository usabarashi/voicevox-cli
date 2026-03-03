use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex, Semaphore};

use crate::interface::mcp_server::protocol::{JsonRpcResponse, INTERNAL_ERROR};
use crate::interface::mcp_server::tools::text_to_speech::spawn_non_send_text_to_speech_task;
use crate::interface::mcp_server::tools::types::{text_result, ToolCallResult};

const MAX_CONCURRENT_TOOL_HANDLERS: usize = 4;

fn serialize_result_response(
    id: Value,
    result: ToolCallResult,
    fallback_message: &str,
) -> JsonRpcResponse {
    match serde_json::to_value(result) {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err(_) => JsonRpcResponse::error(id, INTERNAL_ERROR, fallback_message),
    }
}

fn tool_handler_error_result(error: &anyhow::Error) -> ToolCallResult {
    text_result(format!("Tool handler error: {error}"), true)
}

#[derive(Debug, Clone)]
pub struct ActiveRequests {
    abort_channels: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
    response_sender: mpsc::Sender<JsonRpcResponse>,
    handler_slots: Arc<Semaphore>,
}

impl ActiveRequests {
    #[must_use]
    pub fn new(response_sender: mpsc::Sender<JsonRpcResponse>) -> Self {
        Self {
            abort_channels: Arc::new(Mutex::new(HashMap::new())),
            response_sender,
            handler_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_TOOL_HANDLERS)),
        }
    }

    pub async fn cancel(&self, request_id: &str, reason: Option<String>) -> bool {
        let sender = self.abort_channels.lock().await.remove(request_id);
        sender.is_some_and(|sender| {
            let _ = sender.send(reason.unwrap_or_default());
            true
        })
    }

    pub async fn cancel_all_requests(&self, reason: &str) -> usize {
        let channels = {
            let mut channels = self.abort_channels.lock().await;
            std::mem::take(&mut *channels)
        };
        let count = channels.len();
        let reason = reason.to_string();

        for sender in channels.into_values() {
            let _ = sender.send(reason.clone());
        }

        count
    }

    pub async fn spawn_tool_handler(
        &self,
        request_id: String,
        id: Value,
        tool_name: String,
        arguments: Value,
    ) {
        let permit = match Arc::clone(&self.handler_slots).try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                let response =
                    JsonRpcResponse::error(id, INTERNAL_ERROR, "Too many concurrent tool handlers");
                let _ = self.response_sender.send(response).await;
                return;
            }
        };

        let active_requests = self.clone();
        if tool_name == "text_to_speech" {
            let (abort_tx, abort_rx) = oneshot::channel::<String>();
            self.abort_channels
                .lock()
                .await
                .insert(request_id.clone(), abort_tx);
            spawn_non_send_text_to_speech_task(move || {
                Box::pin(async move {
                    let _permit = permit;
                    let result =
                        crate::interface::mcp_server::tools::registry::execute_tool_request(
                            &tool_name,
                            arguments,
                            Some(abort_rx),
                        )
                        .await;

                    active_requests
                        .abort_channels
                        .lock()
                        .await
                        .remove(&request_id);

                    let response = match result {
                        Ok(tool_result) => serialize_result_response(
                            id,
                            tool_result,
                            "Failed to serialize response",
                        ),
                        Err(error) => serialize_result_response(
                            id,
                            tool_handler_error_result(&error),
                            "Failed to serialize error response",
                        ),
                    };

                    let _ = active_requests.response_sender.send(response).await;
                })
            });
            return;
        }

        tokio::spawn(async move {
            let _permit = permit;
            let result = crate::interface::mcp_server::tools::registry::execute_send_tool_request(
                &tool_name, arguments,
            )
            .await;

            active_requests
                .abort_channels
                .lock()
                .await
                .remove(&request_id);

            let response = match result {
                Ok(tool_result) => {
                    serialize_result_response(id, tool_result, "Failed to serialize response")
                }
                Err(error) => serialize_result_response(
                    id,
                    tool_handler_error_result(&error),
                    "Failed to serialize error response",
                ),
            };

            let _ = active_requests.response_sender.send(response).await;
        });
    }
}
