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
    ParseRequestError, RequestMessage, RequestMethod, parse_request_message,
};

/// Mirrors the spec's `Outcome` sum type.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Outcome {
    Initialize,
    ToolsList,
    ToolsCall,
    Invalid,
    Unknown,
}

fn classify(result: StdResult<RequestMessage, ParseRequestError>) -> Outcome {
    match result {
        Ok(message) => match message.method {
            RequestMethod::Initialize => Outcome::Initialize,
            RequestMethod::ToolsList => Outcome::ToolsList,
            RequestMethod::ToolsCall(_) => Outcome::ToolsCall,
            RequestMethod::Unknown(_) => Outcome::Unknown,
        },
        Err(_) => Outcome::Invalid,
    }
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

    // clippy 1.98 flags quint-connect's `switch!` expansion as `no_effect`.
    #[allow(clippy::no_effect)]
    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            parseInitialize => self.parse(json!({ "id": 1, "method": "initialize" })),
            parseToolsList => self.parse(json!({ "id": 1, "method": "tools/list" })),
            parseToolsCall => self.parse(json!({
                "id": 1,
                "method": "tools/call",
                "params": { "name": "list_voice_styles", "arguments": {} }
            })),
            parseMissingMethod => self.parse(json!({ "id": 1 })),
            parseBadArguments => self.parse(json!({
                "id": 1,
                "method": "tools/call",
                "params": { "name": "list_voice_styles", "arguments": [] }
            })),
            parseMissingParams => self.parse(json!({ "id": 1, "method": "tools/call" })),
            parseNonObjectParams => self.parse(json!({
                "id": 1,
                "method": "tools/call",
                "params": []
            })),
            parseMissingToolName => self.parse(json!({
                "id": 1,
                "method": "tools/call",
                "params": { "arguments": {} }
            })),
            parseUnknownMethod => self.parse(json!({ "id": 1, "method": "unknown/method" })),
            // Fixed scenarios enter through a scenario-specific init action; the
            // observed outcome is still derived from the production parser.
            initMissingMethod => self.parse(json!({ "id": 1 })),
            initBadArguments => self.parse(json!({
                "id": 1,
                "method": "tools/call",
                "params": { "name": "list_voice_styles", "arguments": [] }
            })),
            initUnknownMethod => self.parse(json!({ "id": 1, "method": "unknown/method" })),
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
