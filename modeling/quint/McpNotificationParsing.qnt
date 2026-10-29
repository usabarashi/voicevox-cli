// MCP notification-parsing model.
//
// Mirrors `parse_notification_message` in
// `src/interface/mcp_server/protocol/mod.rs`, which the stdio server uses for
// inbound notifications under the stateless `2026-07-28` revision:
//   * `notifications/cancelled` with a well-formed `requestId` -> Cancelled,
//   * other/unknown notifications (including `cancelled` without valid params)
//     -> Unknown.
//
// `notifications/initialized` belonged to the removed handshake, so it is no
// longer recognized; it falls into `Unknown` like any other stray notification.
//
// Line framing and response correlation stay in `server/stdio.rs` (see
// STATE_TRACEABILITY.md for the coverage boundary).

module McpNotificationParsing {
  type Outcome = Cancelled | Unknown

  var outcome: Outcome

  action init = all {
    outcome' = Unknown,
  }

  action parseCancelled = all {
    outcome' = Cancelled,
  }

  // `cancelled` without a valid `requestId` is treated as unknown.
  action parseCancelledWithoutParams = all {
    outcome' = Unknown,
  }

  // The handshake notification is retired; the server ignores it silently.
  action parseInitialized = all {
    outcome' = Unknown,
  }

  action parseUnknown = all {
    outcome' = Unknown,
  }

  action step = any {
    parseCancelled,
    parseCancelledWithoutParams,
    parseInitialized,
    parseUnknown,
  }
}
