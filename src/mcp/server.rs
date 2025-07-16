use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::mcp::handlers;

/// MCP server manifest
const MCP_VERSION: &str = "0.1.0";

/// Run the MCP server using stdio
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

        // Parse JSON-RPC request
        let request: Value = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("Failed to parse JSON-RPC: {e}");
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32700,
                        "message": "Parse error"
                    },
                    "id": null
                });
                stdout
                    .write_all(serde_json::to_string(&error_response)?.as_bytes())
                    .await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
                continue;
            }
        };

        // Handle request
        let response = if request.get("method").is_some() {
            handle_request(request).await
        } else {
            eprintln!("Invalid JSON-RPC request");
            json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32600,
                    "message": "Invalid request"
                },
                "id": null
            })
        };

        // Send response
        let response_str = serde_json::to_string(&response)?;
        stdout.write_all(response_str.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    eprintln!("EOF on stdin, shutting down MCP server...");
    Ok(())
}

async fn handle_request(request: Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");

    match method {
        "initialize" => {
            let result = json!({
                "protocolVersion": MCP_VERSION,
                "serverInfo": {
                    "name": "voicevox-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "tools": {}
                }
            });

            // Send initialized notification after response
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                let notification = json!({
                    "jsonrpc": "2.0",
                    "method": "initialized"
                });
                if let Ok(notif_str) = serde_json::to_string(&notification) {
                    let mut stdout = tokio::io::stdout();
                    let _ = stdout.write_all(notif_str.as_bytes()).await;
                    let _ = stdout.write_all(b"\n").await;
                    let _ = stdout.flush().await;
                }
            });

            json!({
                "jsonrpc": "2.0",
                "result": result,
                "id": id
            })
        }

        "tools/list" => {
            let tools = json!({
                "tools": [
                    {
                        "name": "text_to_speech",
                        "description": "Convert Japanese text to speech (TTS) and play on server",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "text": {
                                    "type": "string",
                                    "description": "Japanese text to synthesize"
                                },
                                "style_id": {
                                    "type": "integer",
                                    "description": "Voice style ID (e.g., 3 for Zundamon Normal)"
                                },
                                "rate": {
                                    "type": "number",
                                    "description": "Speech rate (0.5-2.0)",
                                    "minimum": 0.5,
                                    "maximum": 2.0,
                                    "default": 1.0
                                },
                                "streaming": {
                                    "type": "boolean",
                                    "description": "Enable streaming playback for lower latency",
                                    "default": true
                                }
                            },
                            "required": ["text", "style_id"]
                        }
                    },
                    {
                        "name": "get_voices",
                        "description": "Get available voices with optional filtering",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "speaker_name": {
                                    "type": "string",
                                    "description": "Filter by speaker name (partial match)"
                                },
                                "style_name": {
                                    "type": "string",
                                    "description": "Filter by style name (partial match)"
                                }
                            }
                        }
                    }
                ]
            });
            json!({
                "jsonrpc": "2.0",
                "result": tools,
                "id": id
            })
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
                                Ok(result) => json!({
                                    "jsonrpc": "2.0",
                                    "result": result,
                                    "id": id
                                }),
                                Err(e) => json!({
                                    "jsonrpc": "2.0",
                                    "error": {
                                        "code": -32603,
                                        "message": format!("Synthesis error: {e}")
                                    },
                                    "id": id
                                }),
                            }
                        }
                        "get_voices" => match handlers::handle_get_voices(arguments).await {
                            Ok(result) => json!({
                                "jsonrpc": "2.0",
                                "result": result,
                                "id": id
                            }),
                            Err(e) => json!({
                                "jsonrpc": "2.0",
                                "error": {
                                    "code": -32603,
                                    "message": format!("Error getting voices: {e}")
                                },
                                "id": id
                            }),
                        },
                        _ => json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32601,
                                "message": format!("Unknown tool: {tool_name}")
                            },
                            "id": id
                        }),
                    }
                } else {
                    json!({
                        "jsonrpc": "2.0",
                        "error": {
                            "code": -32602,
                            "message": "Invalid params"
                        },
                        "id": id
                    })
                }
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "error": {
                        "code": -32602,
                        "message": "Missing params"
                    },
                    "id": id
                })
            }
        }

        _ => json!({
            "jsonrpc": "2.0",
            "error": {
                "code": -32601,
                "message": format!("Method not found: {method}")
            },
            "id": id
        }),
    }
}
