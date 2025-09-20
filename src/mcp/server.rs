use anyhow::Result;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::mcp::protocol::{JsonRpcResponse, INVALID_REQUEST, PARSE_ERROR};
use crate::mcp::requests::ActiveRequests;

pub async fn run_mcp_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Create response channel for async tool execution
    let (response_tx, mut response_rx) = mpsc::unbounded_channel::<JsonRpcResponse>();
    let active_requests = ActiveRequests::new(response_tx);

    let mut shutdown = tokio::spawn(async {
        let _ = tokio::signal::ctrl_c().await;
    });

    loop {
        tokio::select! {
            line_result = lines.next_line() => {
                if !process_line(line_result?, &active_requests, &mut stdout).await {
                    eprintln!("DEBUG: Client disconnected, cancelling all active requests");
                    // Cancel all active requests when client disconnects
                    active_requests.cancel_all_requests("Client disconnected").await;
                    break;
                }
            }
            Some(response) = response_rx.recv() => {
                send_response(&response, &mut stdout).await;
            }
            _ = &mut shutdown => {
                eprintln!("DEBUG: Shutdown signal received, cancelling all active requests");
                active_requests.cancel_all_requests("Server shutdown").await;
                break;
            }
        }
    }

    eprintln!("DEBUG: MCP server shutting down");
    Ok(())
}

async fn process_line(
    line_option: Option<String>,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) -> bool {
    let line = match line_option {
        Some(line) if !line.trim().is_empty() => line,
        Some(_) => return true, // Empty line, continue
        None => return false,   // EOF, terminate
    };

    let raw_request = match parse_json_request(&line, stdout).await {
        Some(request) => request,
        None => return true, // Parse error handled, continue
    };

    if raw_request.get("method").is_some() {
        handle_message(raw_request, active_requests, stdout).await;
    } else {
        send_invalid_request_error(&raw_request, stdout).await;
    }

    true
}

async fn parse_json_request(line: &str, stdout: &mut tokio::io::Stdout) -> Option<Value> {
    match serde_json::from_str(line) {
        Ok(request) => Some(request),
        Err(_) => {
            let id = extract_id_from_invalid_json(line);
            let error_response = JsonRpcResponse::error(id, PARSE_ERROR, "Parse error".to_string());
            send_response(&error_response, stdout).await;
            None
        }
    }
}

fn extract_id_from_invalid_json(line: &str) -> Value {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(Value::Number(serde_json::Number::from(0)))
}

async fn send_invalid_request_error(raw_request: &Value, stdout: &mut tokio::io::Stdout) {
    let id = raw_request
        .get("id")
        .cloned()
        .unwrap_or(Value::Number(serde_json::Number::from(0)));
    let response = JsonRpcResponse::error(id, INVALID_REQUEST, "Invalid request".to_string());
    send_response(&response, stdout).await;
}

async fn send_response(response: &JsonRpcResponse, stdout: &mut tokio::io::Stdout) {
    if let Ok(response_str) = serde_json::to_string(response) {
        let _ = stdout.write_all(response_str.as_bytes()).await;
        let _ = stdout.write_all(b"\n").await;
        let _ = stdout.flush().await;
    }
}

async fn handle_message(
    request: Value,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) {
    // Handle notifications (no response expected)
    if request.get("id").is_none() {
        crate::mcp::protocol::handle_notification(request, active_requests).await;
        return;
    }

    // Handle requests (response expected)
    if let Some(response) = crate::mcp::protocol::process_request(request, active_requests).await {
        send_response(&response, stdout).await;
    }
}
