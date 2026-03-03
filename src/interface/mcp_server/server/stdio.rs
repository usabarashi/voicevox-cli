use anyhow::Result;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, LinesCodec};

use crate::interface::mcp_server::protocol::{
    parse_notification_message, parse_request_message, serialize_success_response,
    InitializeResult, JsonRpcResponse, NotificationMethod, RequestMethod, ServerCapabilities,
    ServerInfo, ToolsListResult, INVALID_REQUEST, METHOD_NOT_FOUND, PARSE_ERROR,
};
use crate::interface::mcp_server::server::runtime::ActiveRequests;
use crate::interface::mcp_server::tools::registry::tool_definitions;

const RESPONSE_QUEUE_CAPACITY: usize = 64;
const MAX_JSONRPC_LINE_BYTES: usize = 256 * 1024;

pub async fn run_stdio_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let codec = LinesCodec::new_with_max_length(MAX_JSONRPC_LINE_BYTES);
    let mut lines = FramedRead::new(stdin, codec);

    let (response_tx, mut response_rx) = mpsc::channel::<JsonRpcResponse>(RESPONSE_QUEUE_CAPACITY);
    let active_requests = ActiveRequests::new(response_tx);

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            frame = lines.next() => {
                match process_frame(frame, &active_requests, &mut stdout).await? {
                    LoopControl::Continue => {}
                    LoopControl::Break => {
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

async fn process_frame(
    frame: Option<Result<String, tokio_util::codec::LinesCodecError>>,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) -> Result<LoopControl> {
    let line = match frame {
        None => return Ok(LoopControl::Break),
        Some(Err(tokio_util::codec::LinesCodecError::MaxLineLengthExceeded)) => {
            let response =
                JsonRpcResponse::error(Value::Null, INVALID_REQUEST, "Request too large");
            send_response(&response, stdout).await?;
            return Ok(LoopControl::Continue);
        }
        Some(Err(e)) => return Err(e.into()),
        Some(Ok(line)) if line.trim().is_empty() => return Ok(LoopControl::Continue),
        Some(Ok(line)) => line,
    };

    let Some(raw_message) = parse_json_request(&line, stdout).await? else {
        return Ok(LoopControl::Continue);
    };

    if raw_message.get("id").is_none() {
        handle_notification(raw_message, active_requests).await;
        return Ok(LoopControl::Continue);
    }

    handle_request(raw_message, active_requests, stdout).await?;
    Ok(LoopControl::Continue)
}

async fn parse_json_request(line: &str, stdout: &mut tokio::io::Stdout) -> Result<Option<Value>> {
    match serde_json::from_str(line) {
        Ok(request) => Ok(Some(request)),
        Err(_) => {
            let error_response = JsonRpcResponse::error(Value::Null, PARSE_ERROR, "Parse error");
            send_response(&error_response, stdout).await?;
            Ok(None)
        }
    }
}

async fn handle_request(
    raw_message: Value,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) -> Result<()> {
    let response_id = raw_message.get("id").cloned().unwrap_or(Value::Null);
    let request = match parse_request_message(raw_message) {
        Ok(request) => request,
        Err(parse_error) => {
            let error_response = parse_error.into_response(response_id);
            send_response(&error_response, stdout).await?;
            return Ok(());
        }
    };

    match request.method {
        RequestMethod::Initialize => {
            let result = InitializeResult {
                protocol_version: crate::interface::mcp_server::protocol::MCP_VERSION.to_string(),
                server_info: ServerInfo {
                    name: "voicevox-mcp".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                capabilities: ServerCapabilities {
                    tools: serde_json::Map::new(),
                },
                instructions: crate::infrastructure::mcp_instructions::load_mcp_instructions(),
            };
            let response = serialize_success_response(request.id, result);
            send_response(&response, stdout).await?;
        }
        RequestMethod::ToolsList => {
            let result = ToolsListResult {
                tools: tool_definitions(),
            };
            let response = serialize_success_response(request.id, result);
            send_response(&response, stdout).await?;
        }
        RequestMethod::ToolsCall(call) => {
            let request_id = match &request.id {
                Value::String(s) => s.to_owned(),
                Value::Number(n) => n.to_string(),
                _ => {
                    let response = JsonRpcResponse::error(
                        request.id,
                        INVALID_REQUEST,
                        "Request id must be a string or number",
                    );
                    send_response(&response, stdout).await?;
                    return Ok(());
                }
            };
            active_requests
                .spawn_tool_handler(request_id, request.id, call.name, call.arguments)
                .await;
        }
        RequestMethod::Unknown(method) => {
            let response = JsonRpcResponse::error(
                request.id,
                METHOD_NOT_FOUND,
                format!("Method not found: {method}"),
            );
            send_response(&response, stdout).await?;
        }
    }

    Ok(())
}

async fn handle_notification(raw_message: Value, active_requests: &ActiveRequests) {
    let notification = parse_notification_message(raw_message);
    match notification.method {
        NotificationMethod::Initialized | NotificationMethod::Unknown => {}
        NotificationMethod::Cancelled(cancelled) => {
            let _ = active_requests
                .cancel(&cancelled.request_id.into_lookup_key(), cancelled.reason)
                .await;
        }
    }
}

async fn send_response(response: &JsonRpcResponse, stdout: &mut tokio::io::Stdout) -> Result<()> {
    let response_str = serde_json::to_string(response)?;
    stdout.write_all(response_str.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}
