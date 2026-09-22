// MCP request lifecycle model.
//
// Mirrors the stdio server's request handling
// (`src/interface/mcp_server/server/runtime.rs` `ActiveRequests` plus the
// dispatch in `server/stdio.rs`):
//   - at most `MAX_CONCURRENT` tool handlers run at once (a semaphore; a
//     request that cannot get a slot is answered with an error immediately),
//   - cancellation only *signals* the handler; the handler keeps its permit
//     until it exits and releases it (`Cancelling` -> `Cancelled`),
//   - `cancelAll` (client disconnect / shutdown) signals every running request.
//
// `MAX_CONCURRENT` is a verification-scale bound (the three IDs must be able to
// exceed it); the production value is `PRODUCTION_MAX_CONCURRENT` and is
// asserted by `EXPECTED_CONSTANTS`.
//
// Duplicate request IDs are not modelled here (production answers a duplicate
// in-flight ID with an error while the original still completes).

module McpRequestLifecycle {
  pure val PRODUCTION_MAX_CONCURRENT = 4
  pure val MAX_CONCURRENT = 2
  pure val REQ_IDS = Set(0, 1, 2)

  type ReqPhase = Unseen | Running | Cancelling | Responded | Cancelled | Rejected

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

  // Cancellation is signalled; the permit is not released yet.
  action cancel = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Running,
      phase' = phase.set(id, Cancelling),
      active' = active,
    }
  }

  // The cancelled handler exits and releases its slot.
  action cancelComplete = all {
    nondet id = oneOf(REQ_IDS)
    all {
      phase.get(id) == Cancelling,
      phase' = phase.set(id, Cancelled),
      active' = active - 1,
    }
  }

  // Client disconnect / server shutdown signals every running request; slots are
  // released as each handler exits.
  action cancelAll = all {
    phase' = phase
      .set(0, if (phase.get(0) == Running) Cancelling else phase.get(0))
      .set(1, if (phase.get(1) == Running) Cancelling else phase.get(1))
      .set(2, if (phase.get(2) == Running) Cancelling else phase.get(2)),
    active' = active,
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
    cancelComplete,
    cancelAll,
    stutter,
  }

  // ---- Safety -------------------------------------------------------------

  val typeOK = active >= 0 and active <= MAX_CONCURRENT

  // Running and cancelling handlers both hold a permit.
  val activeMatchesHolding =
    active ==
      (if (phase.get(0) == Running or phase.get(0) == Cancelling) 1 else 0) +
      (if (phase.get(1) == Running or phase.get(1) == Cancelling) 1 else 0) +
      (if (phase.get(2) == Running or phase.get(2) == Cancelling) 1 else 0)

  // ---- Liveness -----------------------------------------------------------

  action advance = any {
    receive,
    rejectBusy,
    complete,
    cancel,
    cancelComplete,
    cancelAll,
  }

  val vars = (phase, active)

  // A non-empty set of handlers always drains to empty (no stuck handler). The
  // property is NOT `eventually(active == 0)` (which `init` satisfies vacuously).
  temporal allRequestsTerminate =
    weakFair(advance, vars) implies
      always(active > 0 implies eventually(active == 0))
}
