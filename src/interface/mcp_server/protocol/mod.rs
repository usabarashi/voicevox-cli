use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// MCP protocol revision implemented by this server.
///
/// Introduced with revision `2026-07-28`: the protocol is stateless, so every
/// request declares its version and capabilities in `_meta` instead of opening
/// with an `initialize` handshake.
///
/// The wire contract here was checked against the pinned schema at
/// `modelcontextprotocol/modelcontextprotocol` commit
/// `84d845729393f79b3fd3d392aa17cb86f5b6f95c`, file `schema/2026-07-28/schema.json`
/// (sha256 `ef70b61f99b6d2e5e3b46863822eab08dff6a45bedc7a08914e0e5b133f40203`).
/// Pin the commit rather than the `main` path: a dated directory is still
/// mutable on a branch.
pub const MCP_VERSION: &str = "2026-07-28";

/// Protocol versions this server accepts, advertised via `server/discover` and
/// in `UnsupportedProtocolVersionError`.
pub const SUPPORTED_VERSIONS: [&str; 1] = [MCP_VERSION];

pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;
/// Protocol version the server does not implement (MCP-specific range).
pub const UNSUPPORTED_PROTOCOL_VERSION: i32 = -32022;

/// Reserved `_meta` keys carrying per-request protocol fields.
pub const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";
pub const META_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";
/// Reserved `_meta` key carrying the server identity on every result.
pub const META_SERVER_INFO: &str = "io.modelcontextprotocol/serverInfo";

/// Value of the required `resultType` field for ordinary results.
pub const RESULT_TYPE_COMPLETE: &str = "complete";

/// Cache freshness hint for results that never change while the process lives.
pub const STATIC_RESULT_TTL_MS: u64 = 3_600_000;

pub const SERVER_NAME: &str = "voicevox-mcp";

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

/// Self-reported server identity, carried in every result's `_meta`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[must_use]
pub fn server_info() -> Implementation {
    Implementation {
        name: SERVER_NAME.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// `_meta` attached to every result, so clients can identify the server
/// without relying on any prior connection state.
///
/// The field is renamed to the literal `io.modelcontextprotocol/serverInfo`
/// because `serde` derives need a literal; `meta_field_names_match_reserved_keys`
/// keeps it in sync with [`META_SERVER_INFO`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultMeta {
    #[serde(rename = "io.modelcontextprotocol/serverInfo")]
    pub server_info: Implementation,
}

impl Default for ResultMeta {
    fn default() -> Self {
        Self {
            server_info: server_info(),
        }
    }
}

/// Whether a cacheable result may be shared across authorization contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheScope {
    Public,
    Private,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: serde_json::Map<String, Value>,
}

/// Result of `server/discover`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverResult {
    pub result_type: String,
    pub supported_versions: Vec<String>,
    pub capabilities: ServerCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    pub ttl_ms: u64,
    pub cache_scope: CacheScope,
    #[serde(rename = "_meta")]
    pub meta: ResultMeta,
}

/// Result of `tools/list`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult<T> {
    pub result_type: String,
    pub tools: Vec<T>,
    pub ttl_ms: u64,
    pub cache_scope: CacheScope,
    #[serde(rename = "_meta")]
    pub meta: ResultMeta,
}

#[derive(Debug)]
pub enum RequestMethod {
    ServerDiscover,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRequestError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl ParseRequestError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    #[must_use]
    pub fn into_response(self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: self.code,
                message: self.message,
                data: self.data,
            }),
            id,
        }
    }
}

/// Rejection of the retired `initialize` handshake.
///
/// Clients speaking a pre-`2026-07-28` revision have no fall-forward mechanism,
/// so the spec asks a modern-only server to name the versions it supports in
/// whatever error it returns here.
fn unsupported_initialize_error() -> ParseRequestError {
    ParseRequestError {
        code: METHOD_NOT_FOUND,
        message: format!(
            "`initialize` was removed in MCP {MCP_VERSION}; this server speaks only {}. \
             Send the protocol version and client capabilities in `params._meta` on every request.",
            SUPPORTED_VERSIONS.join(", ")
        ),
        data: Some(json!({ "supported": SUPPORTED_VERSIONS })),
    }
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
            Self::String(value) => string_id_key(&value),
            Self::Number(value) => number_id_key(value),
        }
    }
}

/// Internal key for a JSON-RPC request id.
///
/// JSON-RPC ids are typed: `42` and `"42"` are different ids, so the key keeps
/// the type. Flattening them onto the same string would let a cancellation for
/// one request reach another, and would make the in-flight duplicate check
/// reject a second request that uses the other spelling.
fn string_id_key(value: &str) -> String {
    format!("str:{value}")
}

fn number_id_key(value: i64) -> String {
    format!("num:{value}")
}

/// Key under which a request id is tracked, or `None` for a non-id value.
#[must_use]
pub fn request_id_key(id: &Value) -> Option<String> {
    match id {
        Value::String(value) => Some(string_id_key(value)),
        Value::Number(value) => Some(format!("num:{value}")),
        _ => None,
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

pub fn parse_request_message(raw: Value) -> Result<RequestMessage, ParseRequestError> {
    let id = match raw.get("id") {
        None => Value::Null,
        Some(Value::Null) => {
            return Err(ParseRequestError::new(
                INVALID_REQUEST,
                "Invalid request: id must not be null",
            ));
        }
        Some(id) => id.clone(),
    };

    let method = raw
        .get("method")
        .and_then(Value::as_str)
        .ok_or(ParseRequestError::new(
            INVALID_REQUEST,
            "Invalid request: missing method",
        ))?;

    if method == "initialize" {
        return Err(unsupported_initialize_error());
    }

    let params = raw.get("params");
    parse_request_meta(params)?;

    let method = match method {
        "server/discover" => RequestMethod::ServerDiscover,
        "tools/list" => RequestMethod::ToolsList,
        "tools/call" => RequestMethod::ToolsCall(parse_tools_call_params(params)?),
        other => RequestMethod::Unknown(other.to_string()),
    };

    Ok(RequestMessage { id, method })
}

/// Validates the per-request protocol fields that replace the `initialize`
/// handshake.
///
/// `params._meta` must carry `io.modelcontextprotocol/protocolVersion` and
/// `io.modelcontextprotocol/clientCapabilities`. A request missing either is
/// malformed (`-32602`); a well-formed request naming a revision this server
/// does not implement gets `UnsupportedProtocolVersionError` (`-32022`) with the
/// list of supported versions so the client can retry.
fn parse_request_meta(params: Option<&Value>) -> Result<(), ParseRequestError> {
    let Some(Value::Object(params)) = params else {
        return Err(ParseRequestError::new(
            INVALID_PARAMS,
            "Invalid params: expected an object carrying `_meta`",
        ));
    };

    let Some(Value::Object(meta)) = params.get("_meta") else {
        return Err(ParseRequestError::new(
            INVALID_PARAMS,
            format!("Invalid params: missing `_meta` with `{META_PROTOCOL_VERSION}`"),
        ));
    };

    let Some(Value::String(version)) = meta.get(META_PROTOCOL_VERSION) else {
        return Err(ParseRequestError::new(
            INVALID_PARAMS,
            format!("Invalid params: missing `{META_PROTOCOL_VERSION}`"),
        ));
    };

    if !meta
        .get(META_CLIENT_CAPABILITIES)
        .is_some_and(Value::is_object)
    {
        return Err(ParseRequestError::new(
            INVALID_PARAMS,
            format!("Invalid params: missing `{META_CLIENT_CAPABILITIES}` object"),
        ));
    }

    if version != MCP_VERSION {
        return Err(ParseRequestError {
            code: UNSUPPORTED_PROTOCOL_VERSION,
            message: format!(
                "Unsupported protocol version: supported versions are {}",
                SUPPORTED_VERSIONS.join(", ")
            ),
            data: Some(json!({
                "requested": version,
                "supported": SUPPORTED_VERSIONS,
            })),
        });
    }

    Ok(())
}

fn parse_tools_call_params(params: Option<&Value>) -> Result<ToolsCallParams, ParseRequestError> {
    let Some(Value::Object(params_obj)) = params else {
        return Err(ParseRequestError::new(INVALID_PARAMS, "Missing params"));
    };

    let name = params_obj
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(ParseRequestError::new(
            INVALID_PARAMS,
            "Missing or invalid tool name",
        ))?;

    let arguments = match params_obj.get("arguments") {
        None => Value::Object(serde_json::Map::new()),
        Some(Value::Object(arguments)) => Value::Object(arguments.clone()),
        Some(_) => {
            return Err(ParseRequestError::new(
                INVALID_PARAMS,
                "Invalid arguments: expected object",
            ));
        }
    };

    Ok(ToolsCallParams { name, arguments })
}

#[must_use]
pub fn parse_notification_message(raw: Value) -> NotificationMessage {
    let method = raw.get("method").and_then(Value::as_str).unwrap_or("");
    let params = raw.get("params").cloned();

    let method = match method {
        "notifications/cancelled" => params
            .and_then(|value| serde_json::from_value::<CancelledParams>(value).ok())
            .map_or(NotificationMethod::Unknown, NotificationMethod::Cancelled),
        _ => NotificationMethod::Unknown,
    };

    NotificationMessage { method }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    fn modern_params() -> Value {
        json!({ "_meta": { META_PROTOCOL_VERSION: MCP_VERSION, META_CLIENT_CAPABILITIES: {} } })
    }

    #[test]
    fn deserialize_cancelled_params_with_numeric_id() {
        let params = json!({ "requestId": 42 });
        let parsed: CancelledParams = serde_json::from_value(params).expect("should deserialize");
        assert_eq!(parsed.request_id.into_lookup_key(), "num:42");
    }

    #[test]
    fn string_and_numeric_ids_do_not_collide() {
        let numeric = CancelRequestId::Number(42).into_lookup_key();
        let string = CancelRequestId::String("42".to_string()).into_lookup_key();

        assert_ne!(numeric, string, "ids of different JSON types are distinct");
        assert_eq!(
            request_id_key(&json!(42)).as_deref(),
            Some(numeric.as_str())
        );
        assert_eq!(
            request_id_key(&json!("42")).as_deref(),
            Some(string.as_str())
        );
    }

    #[test]
    fn tools_call_rejects_non_object_arguments() {
        let mut params = modern_params();
        params["name"] = json!("list_voice_styles");
        params["arguments"] = json!(["invalid"]);

        let error = parse_request_message(request("tools/call", params))
            .expect_err("expected invalid params");
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("expected object"));
    }

    #[test]
    fn request_without_meta_is_rejected_as_invalid_params() {
        let error = parse_request_message(request("tools/list", json!({})))
            .expect_err("expected invalid params");
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains("_meta"));
    }

    #[test]
    fn request_without_params_is_rejected_as_invalid_params() {
        let raw = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
        let error = parse_request_message(raw).expect_err("expected invalid params");
        assert_eq!(error.code, INVALID_PARAMS);
    }

    #[test]
    fn missing_client_capabilities_is_rejected_as_invalid_params() {
        let params = json!({ "_meta": { META_PROTOCOL_VERSION: MCP_VERSION } });
        let error =
            parse_request_message(request("tools/list", params)).expect_err("expected invalid");
        assert_eq!(error.code, INVALID_PARAMS);
        assert!(error.message.contains(META_CLIENT_CAPABILITIES));
    }

    #[test]
    fn unsupported_version_reports_supported_versions() {
        let params = json!({
            "_meta": { META_PROTOCOL_VERSION: "2025-06-18", META_CLIENT_CAPABILITIES: {} }
        });
        let error =
            parse_request_message(request("tools/list", params)).expect_err("expected unsupported");

        assert_eq!(error.code, UNSUPPORTED_PROTOCOL_VERSION);
        assert_eq!(
            error.data,
            Some(json!({ "requested": "2025-06-18", "supported": SUPPORTED_VERSIONS }))
        );
    }

    #[test]
    fn initialize_names_supported_versions() {
        let error = parse_request_message(request("initialize", json!({})))
            .expect_err("initialize must be rejected");

        assert_eq!(error.code, METHOD_NOT_FOUND);
        assert!(error.message.contains(MCP_VERSION));
        assert_eq!(error.data, Some(json!({ "supported": SUPPORTED_VERSIONS })));
    }

    #[test]
    fn parses_server_discover() {
        let parsed = parse_request_message(request("server/discover", modern_params()))
            .expect("server/discover must parse");
        assert!(matches!(parsed.method, RequestMethod::ServerDiscover));
    }

    #[test]
    fn null_request_id_is_rejected() {
        let raw = json!({ "jsonrpc": "2.0", "id": null, "method": "tools/list" });
        let error = parse_request_message(raw).expect_err("expected invalid request");
        assert_eq!(error.code, INVALID_REQUEST);
    }

    #[test]
    fn initialized_notification_is_no_longer_recognized() {
        let notification =
            parse_notification_message(json!({ "method": "notifications/initialized" }));
        assert!(matches!(notification.method, NotificationMethod::Unknown));
    }

    #[test]
    fn discover_result_carries_result_type_meta_and_cache_hints() {
        let result = DiscoverResult {
            result_type: RESULT_TYPE_COMPLETE.to_string(),
            supported_versions: SUPPORTED_VERSIONS.map(str::to_string).to_vec(),
            capabilities: ServerCapabilities {
                tools: serde_json::Map::new(),
            },
            instructions: None,
            ttl_ms: STATIC_RESULT_TTL_MS,
            cache_scope: CacheScope::Public,
            meta: ResultMeta::default(),
        };

        assert_eq!(
            serde_json::to_value(result).expect("serializes"),
            json!({
                "resultType": "complete",
                "supportedVersions": [MCP_VERSION],
                "capabilities": { "tools": {} },
                "ttlMs": STATIC_RESULT_TTL_MS,
                "cacheScope": "public",
                "_meta": {
                    META_SERVER_INFO: { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") }
                }
            })
        );
    }
}
