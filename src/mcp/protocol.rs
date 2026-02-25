use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::mcp::requests::ActiveRequests;
use crate::mcp::tools::{get_tool_definitions, ToolDefinition};

const MCP_VERSION: &str = "2025-06-18";
const INSTRUCTIONS_ENV_VAR: &str = "VOICEVOX_MCP_INSTRUCTIONS";
const INSTRUCTIONS_FILE: &str = "VOICEVOX.md";

// JSON-RPC 2.0 Protocol Types
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcResponse {
    #[must_use]
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    #[must_use]
    pub fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

fn jsonrpc_error(id: Value, code: i32, message: impl Into<String>) -> JsonRpcResponse {
    JsonRpcResponse::error(id, code, message.into())
}

fn serialize_success_response<T: Serialize>(id: Value, result: T) -> JsonRpcResponse {
    match serde_json::to_value(result) {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err(_) => jsonrpc_error(id, INTERNAL_ERROR, "Failed to serialize response"),
    }
}

fn empty_json_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn take_tool_call_arguments(
    params_obj: &mut serde_json::Map<String, Value>,
) -> Result<Value, String> {
    match params_obj.remove("arguments") {
        None => Ok(empty_json_object()),
        Some(Value::Object(arguments)) => Ok(Value::Object(arguments)),
        Some(_) => Err("Invalid arguments: expected object".to_string()),
    }
}

// JSON-RPC Error Codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// MCP Protocol Types
#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: serde_json::Map<String, Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsListResult {
    pub tools: Vec<ToolDefinition>,
}

/// Parameters for MCP cancellation notifications.
///
/// This structure represents the parameters sent in a `notifications/cancelled` message
/// according to the MCP specification. It provides Rust type safety for the JSON protocol.
///
/// ## MCP Protocol Reference
///
/// See the official MCP cancellation specification:
/// <https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/cancellation>
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CancelRequestId {
    String(String),
    Number(i64),
}

impl CancelRequestId {
    fn into_lookup_key(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelledParams {
    /// The ID of the request to cancel. Must match the `id` field of the original request.
    #[serde(rename = "requestId")]
    pub request_id: CancelRequestId,
    /// Optional human-readable reason for the cancellation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Load MCP server instructions from various locations.
///
/// The instruction loading follows XDG Base Directory compliance with the following priority:
///
/// 1. Environment variable: `VOICEVOX_MCP_INSTRUCTIONS` (highest priority)
/// 2. XDG user config: `$XDG_CONFIG_HOME/voicevox/VOICEVOX.md`
/// 3. Config fallback: `~/.config/voicevox/VOICEVOX.md` (when `XDG_CONFIG_HOME` is not set)
/// 4. Executable directory: `VOICEVOX.md` bundled with the binary (distribution default)
/// 5. Current directory: `VOICEVOX.md` in working directory (development use)
fn load_instructions() -> Option<String> {
    fn try_load(path: &Path) -> Option<String> {
        match fs::read_to_string(path) {
            Ok(content) => Some(content),
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                eprintln!(
                    "Warning: failed to read MCP instructions from {}: {error}",
                    path.display()
                );
                None
            }
            _ => None,
        }
    }

    let xdg_config_home = std::env::var("XDG_CONFIG_HOME").ok();
    let xdg_fallback = xdg_config_home.is_none().then(|| {
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".config").join("voicevox"))
    });

    [
        std::env::var(INSTRUCTIONS_ENV_VAR).ok().map(PathBuf::from),
        xdg_config_home
            .as_ref()
            .map(|path| PathBuf::from(path).join("voicevox").join(INSTRUCTIONS_FILE)),
        xdg_fallback
            .flatten()
            .map(|dir| dir.join(INSTRUCTIONS_FILE)),
        std::env::current_exe()
            .ok()
            .and_then(|exe_path| exe_path.parent().map(|dir| dir.join(INSTRUCTIONS_FILE))),
        Some(PathBuf::from(INSTRUCTIONS_FILE)),
    ]
    .into_iter()
    .flatten()
    .find_map(|path| try_load(&path))
}

/// Initialize request processor - MCP session initialization.
///
/// Establishes the MCP session and returns server capabilities and information.
///
/// ## MCP Protocol Reference
///
/// See the official MCP lifecycle specification:
/// <https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle>
///
/// ## Parameters
///
/// - `id`: Request ID for response correlation
/// - `params`: Initialize parameters (protocol version, capabilities, client info)
///
/// ## Returns
///
/// `InitializeResult` with server info, capabilities, and optional instructions
#[must_use]
pub fn process_initialize(id: Value, _params: Option<Value>) -> JsonRpcResponse {
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

    serialize_success_response(id, result)
}

/// Tools list request processor - Returns available tools.
///
/// Returns a list of all tools provided by this MCP server.
///
/// ## MCP Protocol Reference
///
/// See the official MCP tools specification:
/// <https://modelcontextprotocol.io/specification/2025-06-18/server/tools>
///
/// ## Parameters
///
/// - `id`: Request ID for response correlation
/// - `params`: List parameters (currently unused)
///
/// ## Returns
///
/// `ToolsListResult` containing array of available tool definitions
#[must_use]
pub fn process_tools_list(id: Value, _params: Option<Value>) -> JsonRpcResponse {
    let result = ToolsListResult {
        tools: get_tool_definitions(),
    };

    serialize_success_response(id, result)
}

/// Tools call request processor - Executes a tool.
///
/// Spawns an asynchronous task to execute the requested tool and manages
/// cancellation through the active requests system.
///
/// ## MCP Protocol Reference
///
/// See the official MCP tools specification:
/// <https://modelcontextprotocol.io/specification/2025-06-18/server/tools>
///
/// ## Parameters
///
/// - `id`: Request ID for response correlation and cancellation tracking
/// - `params`: Tool call parameters (name and arguments)
/// - `active_requests`: Request management for cancellation support
///
/// ## Returns
///
/// - `None`: No immediate response (async execution)
/// - `Some(ErrorResponse)`: Parameter validation errors
pub async fn process_tools_call(
    id: Value,
    params: Option<Value>,
    active_requests: &ActiveRequests,
) -> Option<JsonRpcResponse> {
    let request_id = match &id {
        Value::String(s) => s.to_owned(),
        Value::Number(n) => n.to_string(),
        _ => String::from("unknown"),
    };

    let Some(params) = params else {
        return Some(JsonRpcResponse::error(
            id,
            INVALID_PARAMS,
            "Missing params".to_string(),
        ));
    };

    let Value::Object(mut params_obj) = params else {
        return Some(JsonRpcResponse::error(
            id,
            INVALID_PARAMS,
            "Invalid params".to_string(),
        ));
    };

    let Some(tool_name) = params_obj
        .get("name")
        .and_then(|v| v.as_str())
        .map(str::to_owned)
    else {
        return Some(JsonRpcResponse::error(
            id,
            INVALID_PARAMS,
            "Missing or invalid tool name".to_string(),
        ));
    };

    let arguments = match take_tool_call_arguments(&mut params_obj) {
        Ok(arguments) => arguments,
        Err(message) => return Some(JsonRpcResponse::error(id, INVALID_PARAMS, message)),
    };

    active_requests
        .spawn_execution(request_id, id, &tool_name, arguments)
        .await;
    None
}

/// Request dispatcher - Routes MCP requests to specific processors.
///
/// Processes JSON-RPC 2.0 requests (messages with `id` field) and returns
/// appropriate responses. Each request type is processed by a dedicated function.
///
/// ## MCP Protocol Reference
///
/// See the official MCP specification for request handling:
/// <https://modelcontextprotocol.io/specification/2025-06-18/basic/index>
///
/// ## Supported Requests
///
/// - `initialize`: Session initialization
/// - `tools/list`: Tool enumeration
/// - `tools/call`: Tool execution (async)
///
/// ## Parameters
///
/// - `request`: JSON-RPC request with id, method, and optional params
/// - `active_requests`: Request management for cancellation support
///
/// ## Returns
///
/// - `Some(JsonRpcResponse)`: Immediate response
/// - `None`: Async response (tools/call only)
pub async fn process_request(
    request: Value,
    active_requests: &ActiveRequests,
) -> Option<JsonRpcResponse> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let params = request.get("params").cloned();

    match method {
        "initialize" => Some(process_initialize(id, params)),
        "tools/list" => Some(process_tools_list(id, params)),
        "tools/call" => process_tools_call(id, params, active_requests).await,
        _ => Some(jsonrpc_error(
            id,
            METHOD_NOT_FOUND,
            format!("Method not found: {method}"),
        )),
    }
}

/// Handles MCP notifications - messages without id that don't expect responses.
///
/// Dispatches notifications to specific handlers based on the method field.
/// Unknown notifications are silently ignored per MCP specification.
///
/// ## MCP Protocol Reference
///
/// See the official MCP notification specification:
/// <https://modelcontextprotocol.io/specification/2025-06-18/basic/index>
///
/// ## Parameters
///
/// - `notification`: JSON-RPC notification message without id field
/// - `active_requests`: Request management for cancellation support
pub async fn handle_notification(notification: Value, active_requests: &ActiveRequests) {
    let method = notification
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let params = notification.get("params").cloned();

    match method {
        "notifications/initialized" => handle_notification_initialized(params),
        "notifications/cancelled" => handle_notification_cancelled(params, active_requests).await,
        _ => {
            // Unknown notifications are silently ignored per MCP specification
        }
    }
}

/// Initialized notification handler - MCP session confirmation.
///
/// Called when the client sends a `notifications/initialized` message
/// to confirm that the MCP session is ready for operation.
///
/// ## MCP Protocol Reference
///
/// See the official MCP lifecycle specification:
/// <https://modelcontextprotocol.io/specification/2025-06-18/basic/lifecycle>
///
/// ## Parameters
///
/// - `_params`: Notification parameters (currently unused)
fn handle_notification_initialized(_params: Option<Value>) {
    // Currently no action needed for initialized notification
    // This serves as a confirmation that the client is ready
}

/// Cancellation notification handler - MCP request cancellation.
///
/// Processes `notifications/cancelled` messages from the MCP client to cancel
/// actively running requests. Looks up the request by ID and sends the
/// cancellation signal through the associated oneshot channel.
///
/// ## MCP Protocol Reference
///
/// See the official MCP cancellation specification:
/// <https://modelcontextprotocol.io/specification/2025-06-18/basic/utilities/cancellation>
///
/// ## Parameters
///
/// - `params`: Cancellation parameters containing request ID and optional reason
/// - `active_requests`: Request management for sending cancellation signals
async fn handle_notification_cancelled(params: Option<Value>, active_requests: &ActiveRequests) {
    let Some(params) = params else { return };
    let Ok(cancelled_params) = serde_json::from_value::<CancelledParams>(params) else {
        return;
    };

    let CancelledParams { request_id, reason } = cancelled_params;
    let request_id = request_id.into_lookup_key();
    let _ = active_requests.cancel(&request_id, reason).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_cancelled_params_or_panic(params: Value) -> CancelledParams {
        match serde_json::from_value::<CancelledParams>(params) {
            Ok(parsed) => parsed,
            Err(error) => panic!("cancelled params should deserialize: {error}"),
        }
    }

    #[test]
    fn deserialize_cancelled_params_with_numeric_id() {
        let params = json!({
            "requestId": 42,
        });

        let parsed = parse_cancelled_params_or_panic(params);
        assert_eq!(parsed.request_id.into_lookup_key(), "42");
    }

    #[test]
    fn deserialize_cancelled_params_with_string_id() {
        let params = json!({
            "requestId": "abc",
        });

        let parsed = parse_cancelled_params_or_panic(params);
        assert_eq!(parsed.request_id.into_lookup_key(), "abc");
    }

    #[tokio::test]
    async fn tools_call_rejects_missing_tool_name() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let active_requests = ActiveRequests::new(tx);
        let response = process_tools_call(json!(1), Some(json!({})), &active_requests).await;

        let Some(response) = response else {
            panic!("expected immediate invalid-params response");
        };

        let error = response
            .error
            .unwrap_or_else(|| panic!("expected error response"));
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("tool name"));
    }

    #[tokio::test]
    async fn tools_call_rejects_non_object_arguments() {
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let active_requests = ActiveRequests::new(tx);
        let params = json!({
            "name": "list_voice_styles",
            "arguments": ["invalid"]
        });
        let response = process_tools_call(json!(1), Some(params), &active_requests).await;

        let Some(response) = response else {
            panic!("expected immediate invalid-params response");
        };

        let error = response
            .error
            .unwrap_or_else(|| panic!("expected error response"));
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("expected object"));
    }
}
