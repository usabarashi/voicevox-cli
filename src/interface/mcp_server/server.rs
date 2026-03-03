use anyhow::Result;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::interface::mcp_server::protocol::{JsonRpcResponse, INVALID_REQUEST, PARSE_ERROR};
use crate::interface::mcp_server::requests::ActiveRequests;

const RESPONSE_QUEUE_CAPACITY: usize = 64;
const MAX_JSONRPC_LINE_BYTES: usize = 256 * 1024;

/// Runs the MCP server loop over stdio and dispatches JSON-RPC requests/notifications.
///
/// # Errors
///
/// Returns an error if reading stdin lines fails or stdout writes fail while sending
/// responses.
pub async fn run_mcp_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Create response channel for async tool execution
    let (response_tx, mut response_rx) = mpsc::channel::<JsonRpcResponse>(RESPONSE_QUEUE_CAPACITY);
    let active_requests = ActiveRequests::new(response_tx);

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            line_result = lines.next_line() => {
                match process_line(line_result?, &active_requests, &mut stdout).await? {
                    LoopControl::Continue => {}
                    LoopControl::Break => {
                        // Cancel all active requests when client disconnects
                        active_requests.cancel_all_requests("Client disconnected").await;
                        break;
                    }
                }
            }
            Some(response) = response_rx.recv() => {
                if send_response(&response, &mut stdout).await.is_err() {
                    active_requests.cancel_all_requests("Failed to write response").await;
                    break;
                }
            }
            _ = &mut shutdown => {
                active_requests.cancel_all_requests("Server shutdown").await;
                break;
            }
        }
    }

    Ok(())
}

enum LoopControl {
    Continue,
    Break,
}

async fn process_line(
    line_option: Option<String>,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) -> Result<LoopControl> {
    let line = match line_option {
        Some(line) if !line.trim().is_empty() => line,
        Some(_) => return Ok(LoopControl::Continue), // Empty line, continue
        None => return Ok(LoopControl::Break),       // EOF, terminate
    };

    if line.len() > MAX_JSONRPC_LINE_BYTES {
        let error_response = JsonRpcResponse::error(
            Value::Null,
            INVALID_REQUEST,
            "Request too large".to_string(),
        );
        send_response(&error_response, stdout).await?;
        return Ok(LoopControl::Continue);
    }

    let Some(raw_request) = parse_json_request(&line, stdout).await? else {
        return Ok(LoopControl::Continue); // Parse error handled, continue
    };

    if raw_request.get("method").is_some() {
        handle_message(raw_request, active_requests, stdout).await?;
    } else {
        send_invalid_request_error(&raw_request, stdout).await?;
    }

    Ok(LoopControl::Continue)
}

async fn parse_json_request(line: &str, stdout: &mut tokio::io::Stdout) -> Result<Option<Value>> {
    if let Ok(request) = serde_json::from_str(line) {
        Ok(Some(request))
    } else {
        let error_response =
            JsonRpcResponse::error(Value::Null, PARSE_ERROR, "Parse error".to_string());
        send_response(&error_response, stdout).await?;
        Ok(None)
    }
}

async fn send_invalid_request_error(
    raw_request: &Value,
    stdout: &mut tokio::io::Stdout,
) -> Result<()> {
    let id = raw_request.get("id").cloned().unwrap_or(Value::Null);
    let response = JsonRpcResponse::error(id, INVALID_REQUEST, "Invalid request".to_string());
    send_response(&response, stdout).await
}

async fn send_response(response: &JsonRpcResponse, stdout: &mut tokio::io::Stdout) -> Result<()> {
    let response_str = serde_json::to_string(response)?;
    stdout.write_all(response_str.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

async fn handle_message(
    request: Value,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) -> Result<()> {
    // Handle notifications (no response expected)
    if request.get("id").is_none() {
        crate::interface::mcp_server::protocol::handle_notification(request, active_requests).await;
        return Ok(());
    }

    // Handle requests (response expected)
    if let Some(response) =
        crate::interface::mcp_server::protocol::process_request(request, active_requests).await
    {
        send_response(&response, stdout).await?;
    }

    Ok(())
}
