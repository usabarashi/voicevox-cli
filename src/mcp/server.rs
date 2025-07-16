use anyhow::Result;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::handlers;
use crate::mcp::tools::get_tool_definitions;
use crate::mcp::types::*;

const MCP_VERSION: &str = "2025-03-26";

pub async fn run_mcp_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    eprintln!("VOICEVOX MCP Server v{} started", env!("CARGO_PKG_VERSION"));
    eprintln!("Waiting for JSON-RPC messages on stdin...");

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let raw_request: Value = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Failed to parse JSON-RPC: {e}");
                let id = serde_json::from_str::<Value>(&line)
                    .ok()
                    .and_then(|v| v.get("id").cloned())
                    .unwrap_or(Value::Null);
                let error_response =
                    JsonRpcResponse::error(id, PARSE_ERROR, "Parse error".to_string());
                stdout
                    .write_all(serde_json::to_string(&error_response)?.as_bytes())
                    .await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
        };

        let response = if raw_request.get("method").is_some() {
            handle_request(raw_request).await
        } else {
            eprintln!("Invalid JSON-RPC request");
            let id = raw_request.get("id").cloned().unwrap_or(Value::Null);
            JsonRpcResponse::error(id, INVALID_REQUEST, "Invalid request".to_string())
        };

        let response_str = serde_json::to_string(&response)?;
        stdout.write_all(response_str.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    eprintln!("EOF on stdin, shutting down MCP server...");
    Ok(())
}

async fn handle_request(request: Value) -> JsonRpcResponse {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");

    match method {
        "initialize" => {
            let result = InitializeResult {
                protocol_version: MCP_VERSION.to_string(),
                server_info: ServerInfo {
                    name: "voicevox-mcp".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                capabilities: ServerCapabilities {
                    tools: serde_json::Map::new(),
                },
            };

            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let notification = JsonRpcNotification {
                    jsonrpc: "2.0".to_string(),
                    method: "initialized".to_string(),
                    params: None,
                };
                if let Ok(notif_str) = serde_json::to_string(&notification) {
                    let mut stdout = tokio::io::stdout();
                    let _ = stdout.write_all(notif_str.as_bytes()).await;
                    let _ = stdout.write_all(b"\n").await;
                    let _ = stdout.flush().await;
                }
            });

            JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
        }

        "tools/list" => {
            let result = ToolsListResult {
                tools: get_tool_definitions(),
            };
            JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
        }

        "tools/call" => {
            if let Some(params) = request.get("params") {
                if let Some(params_obj) = params.as_object() {
                    let tool_name = params_obj
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    let arguments = params_obj
                        .get("arguments")
                        .cloned()
                        .unwrap_or(Value::Object(serde_json::Map::new()));

                    match tool_name {
                        "text_to_speech" => {
                            match handlers::handle_text_to_speech(arguments).await {
                                Ok(result) => JsonRpcResponse::success(
                                    id.clone(),
                                    serde_json::to_value(result).unwrap(),
                                ),
                                Err(e) => JsonRpcResponse::error(
                                    id.clone(),
                                    INTERNAL_ERROR,
                                    format!("Synthesis error: {e}"),
                                ),
                            }
                        }
                        "get_voices" => match handlers::handle_get_voices(arguments).await {
                            Ok(result) => JsonRpcResponse::success(
                                id.clone(),
                                serde_json::to_value(result).unwrap(),
                            ),
                            Err(e) => JsonRpcResponse::error(
                                id.clone(),
                                INTERNAL_ERROR,
                                format!("Error getting voices: {e}"),
                            ),
                        },
                        _ => JsonRpcResponse::error(
                            id.clone(),
                            METHOD_NOT_FOUND,
                            format!("Unknown tool: {tool_name}"),
                        ),
                    }
                } else {
                    JsonRpcResponse::error(id.clone(), INVALID_PARAMS, "Invalid params".to_string())
                }
            } else {
                JsonRpcResponse::error(id.clone(), INVALID_PARAMS, "Missing params".to_string())
            }
        }

        _ => JsonRpcResponse::error(id, METHOD_NOT_FOUND, format!("Method not found: {method}")),
    }
}
