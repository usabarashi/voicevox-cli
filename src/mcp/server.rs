use anyhow::Result;
use serde_json::Value;
use std::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::handlers;
use crate::mcp::tools::get_tool_definitions;
use crate::mcp::types::*;

const MCP_VERSION: &str = "2025-03-26";
const INSTRUCTIONS_ENV_VAR: &str = "VOICEVOX_MCP_INSTRUCTIONS";
const INSTRUCTIONS_FILE: &str = "INSTRUCTIONS.md";

fn load_instructions() -> Option<String> {
    // 1. Try environment variable first (highest priority)
    if let Ok(custom_path) = std::env::var(INSTRUCTIONS_ENV_VAR) {
        let path = std::path::Path::new(&custom_path);
        match fs::read_to_string(path) {
            Ok(content) => return Some(content),
            Err(e) => {
                eprintln!(
                    "Could not load instructions from environment variable {:?}: {}",
                    path, e
                );
            }
        }
    }

    // 2. Try executable directory (for distributed binaries)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let instructions_path = exe_dir.join(INSTRUCTIONS_FILE);
            match fs::read_to_string(&instructions_path) {
                Ok(content) => return Some(content),
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    eprintln!(
                        "Error loading instructions from {:?}: {}",
                        instructions_path, e
                    );
                }
                _ => {}
            }
        }
    }

    // 3. Fallback: current directory (for development)
    match fs::read_to_string(INSTRUCTIONS_FILE) {
        Ok(content) => Some(content),
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            eprintln!(
                "Error loading instructions from current directory {}: {}",
                INSTRUCTIONS_FILE, e
            );
            None
        }
        _ => None,
    }
}

pub async fn run_mcp_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    let mut shutdown = tokio::spawn(async {
        let _ = tokio::signal::ctrl_c().await;
    });

    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line? {
                    Some(line) if !line.trim().is_empty() => {
                        let raw_request: Value = match serde_json::from_str(&line) {
                            Ok(req) => req,
                            Err(_) => {
                                let id = serde_json::from_str::<Value>(&line)
                                    .ok()
                                    .and_then(|v| v.get("id").cloned())
                                    .unwrap_or(Value::Number(serde_json::Number::from(0)));
                                let error_response =
                                    JsonRpcResponse::error(id, PARSE_ERROR, "Parse error".to_string());
                                if let Ok(response_str) = serde_json::to_string(&error_response) {
                                    let _ = stdout.write_all(response_str.as_bytes()).await;
                                    let _ = stdout.write_all(b"\n").await;
                                    let _ = stdout.flush().await;
                                }
                                continue;
                            }
                        };

                        if raw_request.get("method").is_some() {
                            if let Some(response) = handle_request(raw_request).await {
                                if let Ok(response_str) = serde_json::to_string(&response) {
                                    let _ = stdout.write_all(response_str.as_bytes()).await;
                                    let _ = stdout.write_all(b"\n").await;
                                    let _ = stdout.flush().await;
                                }
                            }
                        } else {
                            let id = raw_request
                                .get("id")
                                .cloned()
                                .unwrap_or(Value::Number(serde_json::Number::from(0)));
                            let response =
                                JsonRpcResponse::error(id, INVALID_REQUEST, "Invalid request".to_string());
                            if let Ok(response_str) = serde_json::to_string(&response) {
                                let _ = stdout.write_all(response_str.as_bytes()).await;
                                let _ = stdout.write_all(b"\n").await;
                                let _ = stdout.flush().await;
                            }
                        }
                    }
                    None => break,
                    _ => continue,
                }
            }
            _ = &mut shutdown => break,
        }
    }

    Ok(())
}

async fn handle_request(request: Value) -> Option<JsonRpcResponse> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");

    match method {
        "initialize" => {
            let id = id.unwrap_or(Value::Number(serde_json::Number::from(0)));
            let result = InitializeResult {
                protocol_version: MCP_VERSION.to_string(),
                server_info: ServerInfo {
                    name: "voicevox-mcp".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                capabilities: ServerCapabilities {
                    tools: serde_json::Map::new(),
                },
                instructions: load_instructions(),
            };

            match serde_json::to_value(result) {
                Ok(value) => Some(JsonRpcResponse::success(id, value)),
                Err(_) => Some(JsonRpcResponse::error(
                    id,
                    INTERNAL_ERROR,
                    "Failed to serialize response".to_string(),
                )),
            }
        }
        "notifications/initialized" => None,
        "tools/list" => {
            let id = id.unwrap_or(Value::Number(serde_json::Number::from(0)));
            let result = ToolsListResult {
                tools: get_tool_definitions(),
            };
            match serde_json::to_value(result) {
                Ok(value) => Some(JsonRpcResponse::success(id, value)),
                Err(_) => Some(JsonRpcResponse::error(
                    id,
                    INTERNAL_ERROR,
                    "Failed to serialize response".to_string(),
                )),
            }
        }
        "tools/call" => {
            let id = id.unwrap_or(Value::Number(serde_json::Number::from(0)));
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
                                Ok(result) => match serde_json::to_value(result) {
                                    Ok(value) => Some(JsonRpcResponse::success(id.clone(), value)),
                                    Err(_) => Some(JsonRpcResponse::error(
                                        id.clone(),
                                        INTERNAL_ERROR,
                                        "Failed to serialize response".to_string(),
                                    )),
                                },
                                Err(e) => {
                                    let error_result = ToolCallResult {
                                        content: vec![ToolContent {
                                            content_type: "text".to_string(),
                                            text: format!("Synthesis error: {e}"),
                                        }],
                                        is_error: Some(true),
                                    };
                                    match serde_json::to_value(error_result) {
                                        Ok(value) => {
                                            Some(JsonRpcResponse::success(id.clone(), value))
                                        }
                                        Err(_) => Some(JsonRpcResponse::error(
                                            id.clone(),
                                            INTERNAL_ERROR,
                                            "Failed to serialize error response".to_string(),
                                        )),
                                    }
                                }
                            }
                        }
                        "list_voice_styles" => {
                            match handlers::handle_list_voice_styles(arguments).await {
                                Ok(result) => match serde_json::to_value(result) {
                                    Ok(value) => Some(JsonRpcResponse::success(id.clone(), value)),
                                    Err(_) => Some(JsonRpcResponse::error(
                                        id.clone(),
                                        INTERNAL_ERROR,
                                        "Failed to serialize response".to_string(),
                                    )),
                                },
                                Err(e) => {
                                    let error_result = ToolCallResult {
                                        content: vec![ToolContent {
                                            content_type: "text".to_string(),
                                            text: format!("Error getting voices: {e}"),
                                        }],
                                        is_error: Some(true),
                                    };
                                    match serde_json::to_value(error_result) {
                                        Ok(value) => {
                                            Some(JsonRpcResponse::success(id.clone(), value))
                                        }
                                        Err(_) => Some(JsonRpcResponse::error(
                                            id.clone(),
                                            INTERNAL_ERROR,
                                            "Failed to serialize error response".to_string(),
                                        )),
                                    }
                                }
                            }
                        }
                        _ => Some(JsonRpcResponse::error(
                            id.clone(),
                            METHOD_NOT_FOUND,
                            format!("Unknown tool: {tool_name}"),
                        )),
                    }
                } else {
                    Some(JsonRpcResponse::error(
                        id.clone(),
                        INVALID_PARAMS,
                        "Invalid params".to_string(),
                    ))
                }
            } else {
                Some(JsonRpcResponse::error(
                    id.clone(),
                    INVALID_PARAMS,
                    "Missing params".to_string(),
                ))
            }
        }
        _ => {
            let id = id.unwrap_or(Value::Number(serde_json::Number::from(0)));
            Some(JsonRpcResponse::error(
                id,
                METHOD_NOT_FOUND,
                format!("Method not found: {method}"),
            ))
        }
    }
}
