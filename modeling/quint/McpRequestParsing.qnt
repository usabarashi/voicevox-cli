// MCP request-parsing model.
//
// Mirrors `parse_request_message` in
// `src/interface/mcp_server/protocol/mod.rs`, which the stdio server uses for
// every inbound JSON-RPC frame:
//   * `initialize`      -> Initialize,
//   * `tools/list`      -> ToolsList,
//   * `tools/call`      -> ToolsCall (valid) or Invalid (missing/`invalid` params),
//   * missing method    -> Invalid,
//   * other methods     -> Unknown.
//
// Line framing (256 KiB limit), notifications/cancellation, and response
// correlation are handled by `server/stdio.rs` and are outside this model (see
// STATE_TRACEABILITY.md for the coverage boundary).

module McpRequestParsing {
  type Outcome = Initialize | ToolsList | ToolsCall | Invalid | Unknown

  var outcome: Outcome

  action init = all {
    outcome' = Invalid,
  }

  action parseInitialize = all {
    outcome' = Initialize,
  }

  action parseToolsList = all {
    outcome' = ToolsList,
  }

  action parseToolsCall = all {
    outcome' = ToolsCall,
  }

  action parseMissingMethod = all {
    outcome' = Invalid,
  }

  action parseBadArguments = all {
    outcome' = Invalid,
  }

  // `tools/call` without params, with non-object params, or without a tool name.
  action parseMissingParams = all {
    outcome' = Invalid,
  }

  action parseNonObjectParams = all {
    outcome' = Invalid,
  }

  action parseMissingToolName = all {
    outcome' = Invalid,
  }

  action parseUnknownMethod = all {
    outcome' = Unknown,
  }

  action step = any {
    parseInitialize,
    parseToolsList,
    parseToolsCall,
    parseMissingMethod,
    parseBadArguments,
    parseMissingParams,
    parseNonObjectParams,
    parseMissingToolName,
    parseUnknownMethod,
  }

  // ---- Fixed regression scenarios -----------------------------------------

  action hold = all {
    outcome' = outcome,
  }

  action initMissingMethod = all {
    outcome' = Invalid,
  }

  action initBadArguments = all {
    outcome' = Invalid,
  }

  action initUnknownMethod = all {
    outcome' = Unknown,
  }
}
