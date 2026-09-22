// MCP server daemon-startup recovery model.
//
// Mirrors `src/interface/mcp_server/startup.rs` `ensure_daemon_running_for_mcp`:
//
//   - the first attempt calls `ensure_daemon_running`:
//       * success (started or already responsive) -> done,
//       * `AlreadyRunning { pid }` -> enter the recovery phase,
//       * any other error -> warn and continue (non-fatal for the MCP server),
//   - the recovery phase terminates the stuck PID and retries once
//     (`recover_stuck_daemon_and_retry`), then finishes (success or failure).
//
// The guarantees modelled here are that recovery is attempted at most once
// (there is no recovery loop) and that the procedure terminates.

module McpStartup {
  type InitialResult = Started | AlreadyRunning | Errored
  type RecoverResult = Recovered | RecoverFailed
  type Phase = Initial | Recovery | Done
  type Outcome = Pending | DidStart | DidRecover | DidFail

  var initialResult: InitialResult
  var phase: Phase
  var outcome: Outcome

  action init = all {
    nondet result = oneOf(Set(Started, AlreadyRunning, Errored))
    all {
      initialResult' = result,
      phase' = Initial,
      outcome' = Pending,
    }
  }

  // The daemon started (or was already responsive).
  action startOk = all {
    phase == Initial,
    initialResult == Started,
    phase' = Done,
    outcome' = DidStart,
    initialResult' = initialResult,
  }

  // Another instance is running: enter the single recovery phase.
  action startAlreadyRunning = all {
    phase == Initial,
    initialResult == AlreadyRunning,
    phase' = Recovery,
    outcome' = Pending,
    initialResult' = initialResult,
  }

  // Any other startup error is non-fatal: warn and serve without a daemon.
  action startErrored = all {
    phase == Initial,
    initialResult == Errored,
    phase' = Done,
    outcome' = DidFail,
    initialResult' = initialResult,
  }

  // Terminate the stuck daemon and retry once, then finish.
  action recover = all {
    phase == Recovery,
    nondet result = oneOf(Set(Recovered, RecoverFailed))
    all {
      phase' = Done,
      outcome' = if (result == Recovered) DidRecover else DidFail,
      initialResult' = initialResult,
    }
  }

  action stutter = all {
    initialResult' = initialResult,
    phase' = phase,
    outcome' = outcome,
  }

  action step = any {
    startOk,
    startAlreadyRunning,
    startErrored,
    recover,
    stutter,
  }

  // ---- Safety -------------------------------------------------------------

  // Finishing always records an outcome (never Done with Pending).
  val doneHasOutcome = phase == Done implies outcome != Pending

  // Recovery is only entered after the first attempt reported AlreadyRunning.
  val recoveryOnlyAfterAlreadyRunning =
    phase == Recovery implies initialResult == AlreadyRunning

  // ---- Liveness -----------------------------------------------------------

  action advance = any {
    startOk,
    startAlreadyRunning,
    startErrored,
    recover,
  }

  val vars = (initialResult, phase, outcome)

  // The startup procedure terminates.
  temporal terminates =
    weakFair(advance, vars) implies eventually(phase == Done)
}
