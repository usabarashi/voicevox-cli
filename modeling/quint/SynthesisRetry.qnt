// Refined Synthesis retry/cancel model (Phase 1, revised).
//
// This is the implementation-facing contract for the **non-streaming**
// client-side synthesis attempt loop, extracted from the MCP retry
// orchestration (`run_retry_loop` in
// `src/interface/mcp_server/tools/text_to_speech.rs`).
//
// Scope: the default MCP path is `streaming = true`, which does **not** use the
// retry loop (see `StreamingSynthesis.qnt`). This model covers only the
// non-streaming daemon path (`handle_daemon_synthesis` -> `run_retry_loop`),
// which is also the path `voicevox-say` uses for its single attempt.
//
// Contract (see CONTRACT.md):
//   - attempts are counted when they *start* (`attempts_started`), including an
//     attempt that is later canceled in flight,
//   - at most MAX_RETRIES retries after the initial attempt,
//   - a backoff is counted when it *starts* (matching `backoffs_started`), and
//     is entered only between attempts,
//   - cancellation is an enabled action from any non-terminal state and is
//     terminal; the model does not assert that cancellation wins over progress
//     (priority is enforced by the orchestration, see CONTRACT.md),
//   - under weak fairness of the progress actions, the loop terminates.
//
// The previous TLA+ model is `modeling/tla/Synthesis.tla`. Differences that
// are intentional contract changes (retry count, liveness strength) are
// recorded in CONTRACT.md.

module SynthesisRetry {
  // Initial attempt + up to MAX_RETRIES retries. Mirrors
  // `MCP_DAEMON_MAX_RETRIES` in `text_to_speech.rs`.
  pure val MAX_RETRIES = 2
  pure val MAX_ATTEMPTS = MAX_RETRIES + 1

  type Outcome = Running | Attempting | Backoff | Done | Failed | Canceled

  var outcome: Outcome

  // Number of synthesis attempts started.
  var attempts: int

  // Number of backoffs started (incremented when a retryable failure is
  // recorded and a retry will follow).
  var backoffs: int

  action init = all {
    outcome' = Running,
    attempts' = 0,
    backoffs' = 0,
  }

  // Start one attempt. The attempt is counted at start, before it can be
  // canceled in flight (production increments `attempts_started` before
  // awaiting the attempt).
  action beginAttempt = all {
    outcome == Running,
    attempts < MAX_ATTEMPTS,
    outcome' = Attempting,
    attempts' = attempts + 1,
    backoffs' = backoffs,
  }

  // The in-flight attempt completes; the environment decides its result. These
  // are separate actions (rather than a nondeterministic result) so that the
  // model-based test can map them onto `RetryPolicy::after_attempt`.
  action attemptOk = all {
    outcome == Attempting,
    outcome' = Done,
    attempts' = attempts,
    backoffs' = backoffs,
  }

  action attemptFatal = all {
    outcome == Attempting,
    outcome' = Failed,
    attempts' = attempts,
    backoffs' = backoffs,
  }

  // A retryable failure starts a backoff only if another attempt is allowed.
  action attemptRetryable = all {
    outcome == Attempting,
    attempts' = attempts,
    backoffs' = if (attempts < MAX_ATTEMPTS) backoffs + 1 else backoffs,
    outcome' = if (attempts >= MAX_ATTEMPTS) Failed else Backoff,
  }

  // The backoff wait completes and the next attempt may start.
  action backoffDone = all {
    outcome == Backoff,
    outcome' = Running,
    attempts' = attempts,
    backoffs' = backoffs,
  }

  // Cancellation is modelled as an enabled action from a pending state. The
  // model deliberately has no "cancel requested" state, so it does not assert
  // that cancellation wins over progress; that priority is an orchestration
  // concern enforced by the implementation's checkpoints (see CONTRACT.md).
  action cancel = all {
    outcome == Running or outcome == Attempting or outcome == Backoff,
    outcome' = Canceled,
    attempts' = attempts,
    backoffs' = backoffs,
  }

  action step = any {
    beginAttempt,
    attemptOk,
    attemptFatal,
    attemptRetryable,
    backoffDone,
    cancel,
  }

  // ---- Safety --------------------------------------------------------------

  // Total attempts never exceed the contract bound.
  val attemptsBounded = attempts <= MAX_ATTEMPTS

  // Backoffs never exceed MAX_RETRIES, and only happen after an attempt.
  val backoffsBounded = backoffs <= MAX_RETRIES

  // A backoff only happens between attempts.
  val backoffAfterAttempt = outcome == Backoff implies attempts >= 1

  // ---- Liveness ------------------------------------------------------------

  action advance = any {
    beginAttempt,
    attemptOk,
    attemptFatal,
    attemptRetryable,
    backoffDone,
  }

  // Variable tuple used by the fairness assumption (TLA+ `vars`).
  val vars = (outcome, attempts, backoffs)

  // If the loop keeps making progress opportunities, it eventually reaches a
  // terminal outcome. Weak fairness is required: `advance` is continuously
  // enabled while Running (with attempts remaining), Attempting, or Backoff.
  temporal eventuallyTerminal =
    weakFair(advance, vars) implies
      eventually(
        outcome == Done or outcome == Failed or outcome == Canceled
      )

  // Cancellation is terminal: no action is enabled from `Canceled`.
  temporal cancelIsTerminal =
    always(outcome == Canceled implies always(outcome == Canceled))

  // ---- Fixed scenarios (production retry-loop MBT) ------------------------
  //
  // Terminal states that the production `run_retry_loop` must produce for the
  // scripted attempt/waiter sequences used by
  // `mbt/tests/synthesis_retry_loop.rs`. The driver runs the real loop with
  // controllable seams and compares its observed counters against these states.

  action hold = all {
    outcome' = outcome,
    attempts' = attempts,
    backoffs' = backoffs,
  }

  // Three retryable failures: attempts exhausted, two backoffs started.
  action scenarioExhaustRetryable = all {
    outcome' = Failed,
    attempts' = MAX_ATTEMPTS,
    backoffs' = MAX_RETRIES,
  }

  // Retryable failure, backoff, then success.
  action scenarioSuccessAfterRetry = all {
    outcome' = Done,
    attempts' = 2,
    backoffs' = 1,
  }

  // Non-retryable failure on the first attempt.
  action scenarioFatalFirst = all {
    outcome' = Failed,
    attempts' = 1,
    backoffs' = 0,
  }

  // Cancellation delivered before the first attempt starts.
  action scenarioCancelBeforeFirstAttempt = all {
    outcome' = Canceled,
    attempts' = 0,
    backoffs' = 0,
  }

  // Cancellation delivered while waiting for the first backoff.
  action scenarioCancelDuringBackoff = all {
    outcome' = Canceled,
    attempts' = 1,
    backoffs' = 1,
  }

  // Cancellation during the third in-flight attempt.
  action scenarioCancelInFlightThird = all {
    outcome' = Canceled,
    attempts' = MAX_ATTEMPTS,
    backoffs' = MAX_RETRIES,
  }
}
