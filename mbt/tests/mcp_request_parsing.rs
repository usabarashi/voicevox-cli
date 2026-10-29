//! Model-based test for MCP request parsing.
//!
//! The driver calls the **production** `parse_request_message` with one fixture
//! per action and lets Quint Connect compare the observed outcome against
//! `modeling/quint/McpRequestParsing.qnt`.
//!
//! This covers the request-routing boundary of the MCP server; line framing,
//! notifications, and response correlation are separate (see
//! STATE_TRACEABILITY.md).

use anyhow::Context;
use quint_connect::*;
use serde::Deserialize;
use serde_json::json;
use std::result::Result as StdResult;
use voicevox_cli::interface::mcp_server::protocol::{
    MCP_VERSION, META_CLIENT_CAPABILITIES, META_PROTOCOL_VERSION, ParseRequestError,
    RequestMessage, RequestMethod, UNSUPPORTED_PROTOCOL_VERSION, parse_request_message,
};

/// Mirrors the spec's `Outcome` sum type.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Outcome {
    ServerDiscover,
    ToolsList,
    ToolsCall,
    UnsupportedVersion,
    Invalid,
    Unknown,
}

fn classify(result: StdResult<RequestMessage, ParseRequestError>) -> Outcome {
    match result {
        Ok(message) => match message.method {
            RequestMethod::ServerDiscover => Outcome::ServerDiscover,
            RequestMethod::ToolsList => Outcome::ToolsList,
            RequestMethod::ToolsCall(_) => Outcome::ToolsCall,
            RequestMethod::Unknown(_) => Outcome::Unknown,
        },
        Err(error) if error.code == UNSUPPORTED_PROTOCOL_VERSION => Outcome::UnsupportedVersion,
        Err(_) => Outcome::Invalid,
    }
}

/// `_meta` for a request that speaks the version this server implements.
fn modern_params() -> serde_json::Value {
    json!({ "_meta": { META_PROTOCOL_VERSION: MCP_VERSION, META_CLIENT_CAPABILITIES: {} } })
}

fn request(method: &str, params: serde_json::Value) -> serde_json::Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
}

#[derive(Default)]
struct ParsingDriver {
    outcome: Option<Outcome>,
}

impl ParsingDriver {
    fn reset(&mut self) {
        // Mirrors the spec's `init`.
        self.outcome = Some(Outcome::Invalid);
    }

    fn parse(&mut self, raw: serde_json::Value) {
        self.outcome = Some(classify(parse_request_message(raw)));
    }

    /// Parses a request whose `jsonrpc` member is `value` instead of `"2.0"`.
    fn parse_jsonrpc(&mut self, value: serde_json::Value) {
        let mut raw = request("tools/list", modern_params());
        raw["jsonrpc"] = value;
        self.parse(raw);
    }

    /// Parses a request carrying `id` instead of a valid one.
    fn parse_id(&mut self, value: serde_json::Value) {
        let mut raw = request("tools/list", modern_params());
        raw["id"] = value;
        self.parse(raw);
    }
}

impl State<ParsingDriver> for Outcome {
    fn from_driver(driver: &ParsingDriver) -> Result<Self> {
        driver.outcome.clone().context("outcome not set")
    }
}

impl Driver for ParsingDriver {
    type State = Outcome;

    fn config() -> Config {
        Config {
            state: &["outcome"],
            ..Config::default()
        }
    }

    #[allow(clippy::no_effect)] // quint-connect's `switch!` macro
    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            parseServerDiscover => self.parse(request("server/discover", modern_params())),
            parseToolsList => self.parse(request("tools/list", modern_params())),
            parseToolsCall => self.parse(request(
                "tools/call",
                json!({
                    "_meta": {
                        META_PROTOCOL_VERSION: MCP_VERSION,
                        META_CLIENT_CAPABILITIES: {}
                    },
                    "name": "list_voice_styles",
                    "arguments": {}
                }),
            )),
            // The `initialize` handshake was removed; a legacy client gets an
            // error naming the supported versions.
            parseInitialize => self.parse(request("initialize", json!({}))),
            parseMissingMeta => self.parse(request("tools/list", json!({}))),
            parseMissingCapabilities => self.parse(request(
                "tools/list",
                json!({ "_meta": { META_PROTOCOL_VERSION: MCP_VERSION } }),
            )),
            parseUnsupportedVersion => self.parse(request(
                "tools/list",
                json!({
                    "_meta": {
                        META_PROTOCOL_VERSION: "2025-06-18",
                        META_CLIENT_CAPABILITIES: {}
                    }
                }),
            )),
            parseMissingMethod => self.parse(json!({ "id": 1 })),
            parseBadJsonrpc => self.parse_jsonrpc(json!("1.0")),
            parseBadId => self.parse_id(json!({ "nested": 1 })),
            parseBadArguments => self.parse(request(
                "tools/call",
                json!({
                    "_meta": {
                        META_PROTOCOL_VERSION: MCP_VERSION,
                        META_CLIENT_CAPABILITIES: {}
                    },
                    "name": "list_voice_styles",
                    "arguments": []
                }),
            )),
            parseMissingParams => self.parse(json!({ "id": 1, "method": "tools/call" })),
            parseNonObjectParams => self.parse(request("tools/call", json!([]))),
            parseMissingToolName => self.parse(request("tools/call", modern_params())),
            parseUnknownMethod => self.parse(request("unknown/method", modern_params())),
            // Fixed scenarios enter through a scenario-specific init action; the
            // observed outcome is still derived from the production parser.
            initMissingMethod => self.parse(json!({ "id": 1 })),
            initBadArguments => self.parse(request(
                "tools/call",
                json!({
                    "_meta": {
                        META_PROTOCOL_VERSION: MCP_VERSION,
                        META_CLIENT_CAPABILITIES: {}
                    },
                    "name": "list_voice_styles",
                    "arguments": []
                }),
            )),
            initUnknownMethod => self.parse(request("unknown/method", modern_params())),
            initInitialize => self.parse(request("initialize", json!({}))),
            initMissingMeta => self.parse(request("tools/list", json!({}))),
            initUnsupportedVersion => self.parse(request(
                "tools/list",
                json!({
                    "_meta": {
                        META_PROTOCOL_VERSION: "2025-06-18",
                        META_CLIENT_CAPABILITIES: {}
                    }
                }),
            )),
            initBadJsonrpc => self.parse_jsonrpc(json!("1.0")),
            initBadId => self.parse_id(json!(null)),
            hold => (),
            _ => (),
        })
    }
}

/// Random exploration of the request-parsing outcomes.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    max_samples = 400,
    max_steps = 8
)]
fn mcp_request_parsing_simulation() -> impl Driver {
    ParsingDriver::default()
}

/// Fixed regression scenario: a request without a method is invalid.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    init = "initMissingMethod",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_request_parsing_missing_method() -> impl Driver {
    ParsingDriver::default()
}

/// Fixed regression scenario: non-object `tools/call` arguments are invalid.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    init = "initBadArguments",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_request_parsing_bad_arguments() -> impl Driver {
    ParsingDriver::default()
}

/// Fixed regression scenario: an unknown method is reported as unknown.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    init = "initUnknownMethod",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_request_parsing_unknown_method() -> impl Driver {
    ParsingDriver::default()
}

/// Fixed regression scenario: the retired `initialize` handshake is rejected.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    init = "initInitialize",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_request_parsing_initialize_is_removed() -> impl Driver {
    ParsingDriver::default()
}

/// Fixed regression scenario: a request without `_meta` is malformed, not an
/// unsupported version.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    init = "initMissingMeta",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_request_parsing_missing_meta() -> impl Driver {
    ParsingDriver::default()
}

/// Fixed regression scenario: a well-formed `_meta` naming an unimplemented
/// version reports it separately so the client can retry.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    init = "initUnsupportedVersion",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_request_parsing_unsupported_version() -> impl Driver {
    ParsingDriver::default()
}

/// Fixed regression scenario: a frame that is not JSON-RPC 2.0 is rejected
/// before its method is dispatched.
#[quint_run(
    spec = "../modeling/quint/McpRequestParsing.qnt",
    init = "initBadJsonrpc",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_request_parsing_bad_jsonrpc() -> impl Driver {
    ParsingDriver::default()
}
