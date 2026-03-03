use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MCP_VERSION: &str = "2025-06-18";

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

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
pub struct ToolsListResult<T> {
    pub tools: Vec<T>,
}

#[derive(Debug)]
pub enum RequestMethod {
    Initialize,
    ToolsList,
    ToolsCall(ToolsCallParams),
    Unknown(String),
}

#[derive(Debug)]
pub struct RequestMessage {
    pub id: Value,
    pub method: RequestMethod,
}

#[derive(Debug)]
pub enum NotificationMethod {
    Initialized,
    Cancelled(CancelledParams),
    Unknown,
}

#[derive(Debug)]
pub struct NotificationMessage {
    pub method: NotificationMethod,
}

#[derive(Debug)]
pub struct ToolsCallParams {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CancelRequestId {
    String(String),
    Number(i64),
}

impl CancelRequestId {
    #[must_use]
    pub fn into_lookup_key(self) -> String {
        match self {
            Self::String(value) => value,
            Self::Number(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelledParams {
    #[serde(rename = "requestId")]
    pub request_id: CancelRequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    pub fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }

    #[must_use]
    pub fn internal_error(id: Value, message: &str) -> Self {
        Self::error(id, INTERNAL_ERROR, message)
    }
}

#[must_use]
pub fn serialize_success_response<T: Serialize>(id: Value, result: T) -> JsonRpcResponse {
    match serde_json::to_value(result) {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err(_) => JsonRpcResponse::internal_error(id, "Failed to serialize response"),
    }
}

#[must_use]
pub fn parse_request_message(raw: Value) -> Result<RequestMessage, JsonRpcResponse> {
    let id = raw.get("id").cloned().unwrap_or(Value::Null);
    let method = match raw.get("method").and_then(Value::as_str) {
        Some(method) => method,
        None => {
            return Err(JsonRpcResponse::error(
                id,
                INVALID_REQUEST,
                "Invalid request: missing method",
            ))
        }
    };

    let params = raw.get("params").cloned();
    let method = match method {
        "initialize" => RequestMethod::Initialize,
        "tools/list" => RequestMethod::ToolsList,
        "tools/call" => RequestMethod::ToolsCall(parse_tools_call_params(id.clone(), params)?),
        other => RequestMethod::Unknown(other.to_string()),
    };

    Ok(RequestMessage { id, method })
}

fn parse_tools_call_params(
    id: Value,
    params: Option<Value>,
) -> Result<ToolsCallParams, JsonRpcResponse> {
    let Some(params) = params else {
        return Err(JsonRpcResponse::error(id, INVALID_PARAMS, "Missing params"));
    };

    let Value::Object(mut params_obj) = params else {
        return Err(JsonRpcResponse::error(id, INVALID_PARAMS, "Invalid params"));
    };

    let Some(name) = params_obj
        .remove("name")
        .and_then(|v| v.as_str().map(str::to_owned))
    else {
        return Err(JsonRpcResponse::error(
            id,
            INVALID_PARAMS,
            "Missing or invalid tool name",
        ));
    };

    let arguments = match params_obj.remove("arguments") {
        None => Value::Object(serde_json::Map::new()),
        Some(Value::Object(arguments)) => Value::Object(arguments),
        Some(_) => {
            return Err(JsonRpcResponse::error(
                id,
                INVALID_PARAMS,
                "Invalid arguments: expected object",
            ))
        }
    };

    Ok(ToolsCallParams { name, arguments })
}

#[must_use]
pub fn parse_notification_message(raw: Value) -> NotificationMessage {
    let method = raw.get("method").and_then(Value::as_str).unwrap_or("");
    let params = raw.get("params").cloned();

    let method = match method {
        "notifications/initialized" => NotificationMethod::Initialized,
        "notifications/cancelled" => {
            match params.and_then(|value| serde_json::from_value::<CancelledParams>(value).ok()) {
                Some(cancelled) => NotificationMethod::Cancelled(cancelled),
                None => NotificationMethod::Unknown,
            }
        }
        _ => NotificationMethod::Unknown,
    };

    NotificationMessage { method }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialize_cancelled_params_with_numeric_id() {
        let params = json!({ "requestId": 42 });
        let parsed: CancelledParams = serde_json::from_value(params).expect("should deserialize");
        assert_eq!(parsed.request_id.into_lookup_key(), "42");
    }

    #[test]
    fn tools_call_rejects_non_object_arguments() {
        let raw = json!({
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "list_voice_styles",
                "arguments": ["invalid"]
            }
        });

        let error = parse_request_message(raw).expect_err("expected invalid params");
        let err = error.error.expect("error payload required");
        assert_eq!(err.code, INVALID_PARAMS);
        assert!(err.message.contains("expected object"));
    }
}
