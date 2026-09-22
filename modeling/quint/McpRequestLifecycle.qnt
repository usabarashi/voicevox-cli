// MCP request lifecycle model.
//
// Mirrors the stdio server's request handling
// (`src/interface/mcp_server/server/runtime.rs` `ActiveRequests` plus the
// dispatch in `server/stdio.rs`):
//   - at most `MAX_CONCURRENT` tool handlers run at once (a semaphore; a
//     request that cannot get a slot is answered with an error immediately),
//   - a running request terminates by completion or cancellation,
//   - `cancelAll` (client disconnect / shutdown) cancels every running request.
//
// Duplicate request IDs are not modelled here (production answers a duplicate
// in-flight ID with an error while the original still completes; that
// double-response is a separate concern).

module McpRequestLifecycle {
  pure val MAX_CONCURRENT = 4
  pure val REQ_IDS = Set(0, 1, 2)

  type ReqPhase = Unseen | Running | Responded | Cancelled | Rejected

  var phase: int -> ReqPhase
  var active: int

  action init = all {
    phase' = Map(0 -> Unseen, 1 -> Unseen, 2 -> Unseen),
    active' = 0,
  }

  // Admit a new request into a handler slot.
  action receive = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Unseen,
      active < MAX_CONCURRENT,
      phase' = phase.set(id, Running),
      active' = active + 1,
    }
  }

  // No slot is available: the request is answered with an error without
  // entering a handler.
  action rejectBusy = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Unseen,
      active == MAX_CONCURRENT,
      phase' = phase.set(id, Rejected),
      active' = active,
    }
  }

  action complete = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Running,
      phase' = phase.set(id, Responded),
      active' = active - 1,
    }
  }

  action cancel = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Running,
      phase' = phase.set(id, Cancelled),
      active' = active - 1,
    }
  }

  // Client disconnect / server shutdown cancels every running request.
  action cancelAll = all {
    phase' = phase
      .set(0, if (phase.get(0) == Running) Cancelled else phase.get(0))
      .set(1, if (phase.get(1) == Running) Cancelled else phase.get(1))
      .set(2, if (phase.get(2) == Running) Cancelled else phase.get(2)),
    active' = 0,
  }

  action stutter = all {
    phase' = phase,
    active' = active,
  }

  action step = any {
    receive,
    rejectBusy,
    complete,
    cancel,
    cancelAll,
    stutter,
  }

  // ---- Safety -------------------------------------------------------------

  val typeOK = active >= 0 and active <= MAX_CONCURRENT

  val activeMatchesRunning =
    active ==
      (if (phase.get(0) == Running) 1 else 0) +
      (if (phase.get(1) == Running) 1 else 0) +
      (if (phase.get(2) == Running) 1 else 0)

  // ---- Liveness -----------------------------------------------------------

  action advance = any {
    receive,
    rejectBusy,
    complete,
    cancel,
    cancelAll,
  }

  val vars = (phase, active)

  // Each ID can be received at most once, so under fairness every request
  // eventually reaches a terminal phase (no handler is stuck running).
  temporal allRequestsTerminate =
    weakFair(advance, vars) implies eventually(active == 0)
}
