// MCP request-parsing model.
//
// Mirrors `parse_request_message` in
// `src/interface/mcp_server/protocol/mod.rs`, which the stdio server uses for
// every inbound JSON-RPC frame under the stateless `2026-07-28` revision:
//   * `server/discover`          -> ServerDiscover,
//   * `tools/list`               -> ToolsList,
//   * `tools/call`               -> ToolsCall (valid) or Invalid (bad params),
//   * `initialize`               -> Invalid (the handshake was removed),
//   * well-formed `_meta` naming
//     a version this server does
//     not implement              -> UnsupportedVersion (client may retry),
//   * missing or malformed
//     `_meta`                    -> Invalid (malformed request),
//   * `jsonrpc` other than "2.0" -> Invalid (not a JSON-RPC 2.0 request),
//   * `id` that is not a string
//     or number (or is null)     -> Invalid,
//   * missing method             -> Invalid,
//   * other methods              -> Unknown.
//
// `UnsupportedVersion` is kept distinct from `Invalid` because the two drive
// different client behaviour: a version mismatch is retryable against a listed
// version, while a malformed request is not.
//
// Line framing (256 KiB limit), notifications/cancellation, and response
// correlation are handled by `server/stdio.rs` and are outside this model (see
// STATE_TRACEABILITY.md for the coverage boundary).

module McpRequestParsing {
  type Outcome =
    | ServerDiscover
    | ToolsList
    | ToolsCall
    | UnsupportedVersion
    | Invalid
    | Unknown

  var outcome: Outcome

  action init = all {
    outcome' = Invalid,
  }

  action parseServerDiscover = all {
    outcome' = ServerDiscover,
  }

  action parseToolsList = all {
    outcome' = ToolsList,
  }

  action parseToolsCall = all {
    outcome' = ToolsCall,
  }

  // The `initialize` handshake is not part of this revision.
  action parseInitialize = all {
    outcome' = Invalid,
  }

  action parseMissingMeta = all {
    outcome' = Invalid,
  }

  action parseMissingCapabilities = all {
    outcome' = Invalid,
  }

  action parseUnsupportedVersion = all {
    outcome' = UnsupportedVersion,
  }

  action parseMissingMethod = all {
    outcome' = Invalid,
  }

  // The `jsonrpc` member is validated before anything else.
  action parseBadJsonrpc = all {
    outcome' = Invalid,
  }

  // JSON-RPC ids are strings or numbers, never null, objects or booleans.
  action parseBadId = all {
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
    parseServerDiscover,
    parseToolsList,
    parseToolsCall,
    parseInitialize,
    parseMissingMeta,
    parseMissingCapabilities,
    parseUnsupportedVersion,
    parseMissingMethod,
    parseBadJsonrpc,
    parseBadId,
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

  // A client that still opens with `initialize` is answered with an error that
  // names the supported versions.
  action initInitialize = all {
    outcome' = Invalid,
  }

  // A request without `_meta` is malformed and must not be treated as a version
  // mismatch.
  action initMissingMeta = all {
    outcome' = Invalid,
  }

  action initUnsupportedVersion = all {
    outcome' = UnsupportedVersion,
  }

  // A frame that does not declare JSON-RPC 2.0 is not a valid request.
  action initBadJsonrpc = all {
    outcome' = Invalid,
  }

  action initBadId = all {
    outcome' = Invalid,
  }
}
