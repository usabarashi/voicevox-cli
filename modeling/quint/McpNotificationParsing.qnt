// MCP notification-parsing model.
//
// Mirrors `parse_notification_message` in
// `src/interface/mcp_server/protocol/mod.rs`, which the stdio server uses for
// inbound notifications (notably `notifications/cancelled`, which routes a
// cancel to an in-flight tool request):
//   * `notifications/initialized` -> Initialized,
//   * `notifications/cancelled` with a well-formed `requestId` -> Cancelled,
//   * other/unknown notifications (including `cancelled` without valid params)
//     -> Unknown.
//
// Line framing and response correlation stay in `server/stdio.rs` (see
// STATE_TRACEABILITY.md for the coverage boundary).

module McpNotificationParsing {
  type Outcome = Initialized | Cancelled | Unknown

  var outcome: Outcome

  action init = all {
    outcome' = Unknown,
  }

  action parseInitialized = all {
    outcome' = Initialized,
  }

  action parseCancelled = all {
    outcome' = Cancelled,
  }

  // `cancelled` without a valid `requestId` is treated as unknown.
  action parseCancelledWithoutParams = all {
    outcome' = Unknown,
  }

  action parseUnknown = all {
    outcome' = Unknown,
  }

  action step = any {
    parseInitialized,
    parseCancelled,
    parseCancelledWithoutParams,
    parseUnknown,
  }
}
