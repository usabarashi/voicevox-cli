// Refined Synthesis retry/cancel model (Phase 1).
//
// This is the implementation-facing contract for the client-side synthesis
// attempt loop, extracted from the MCP retry orchestration
// (`run_daemon_retry_phase` in
// `src/interface/mcp_server/tools/text_to_speech.rs`).
//
// Contract (see CONTRACT.md):
//   - at most MAX_RETRIES retries after the initial attempt,
//   - backoff is entered only between attempts,
//   - cancellation is accepted at any checkpoint and takes priority,
//   - under weak fairness of the progress actions, the loop terminates.
//
// The previous TLA+ model is `modeling/tla/Synthesis.tla`. Differences that
// are intentional contract changes (retry count, liveness strength) are
// recorded in CONTRACT.md.

module SynthesisRetry {
  // Initial attempt + up to MAX_RETRIES retries.
  pure val MAX_RETRIES = 2
  pure val MAX_ATTEMPTS = MAX_RETRIES + 1

  type Outcome = Running | Backoff | Done | Failed | Canceled

  var outcome: Outcome

  // Number of synthesis attempts started.
  var attempts: int

  // Number of backoffs entered.
  var backoffs: int

  action init = all {
    outcome' = Running,
    attempts' = 0,
    backoffs' = 0,
  }

  // Start one attempt; the environment decides its result.
  action attempt = all {
    outcome == Running,
    attempts < MAX_ATTEMPTS,
    nondet result = Set("ok", "retryable", "fatal").oneOf()
    all {
      attempts' = attempts + 1,
      backoffs' = backoffs,
      outcome' =
        if (result == "ok") Done
        else if (result == "fatal") Failed
        else if (attempts + 1 >= MAX_ATTEMPTS) Failed
        else Backoff,
    }
  }

  // The backoff wait completes and the next attempt may start.
  action backoffDone = all {
    outcome == Backoff,
    outcome' = Running,
    attempts' = attempts,
    backoffs' = backoffs + 1,
  }

  // Cancellation is accepted at any checkpoint and wins over progress.
  action cancel = all {
    outcome == Running or outcome == Backoff,
    outcome' = Canceled,
    attempts' = attempts,
    backoffs' = backoffs,
  }

  action step = any {
    attempt,
    backoffDone,
    cancel,
  }

  // ---- Safety --------------------------------------------------------------

  // Total attempts never exceed the contract bound.
  val attemptsBounded = attempts <= MAX_ATTEMPTS

  // Retries (backoffs) never exceed MAX_RETRIES.
  val backoffsBounded = backoffs <= MAX_RETRIES

  // A backoff only happens between attempts.
  val backoffAfterAttempt = outcome == Backoff implies attempts >= 1

  // ---- Liveness ------------------------------------------------------------

  action advance = any {
    attempt,
    backoffDone,
  }

  // Variable tuple used by the fairness assumption (TLA+ `vars`).
  val vars = (outcome, attempts, backoffs)

  // If the loop keeps making progress opportunities, it eventually reaches a
  // terminal outcome. Weak fairness is required: `advance` is continuously
  // enabled while Running (with attempts remaining) or Backoff.
  temporal eventuallyTerminal =
    weakFair(advance, vars) implies
      eventually(
        outcome == Done or outcome == Failed or outcome == Canceled
      )
}
