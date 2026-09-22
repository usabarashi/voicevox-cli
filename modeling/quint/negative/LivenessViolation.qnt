// Negative control: progress can always be postponed (an always-enabled
// skip), so without a fairness assumption the terminal outcome is never
// guaranteed. The `eventuallyTerminal` liveness property must be violated.
// Used by verify.sh to prove that the TLC backend detects an infinite stall,
// not just a deadlock.

module LivenessViolation {
  type Outcome = Running | Done

  var outcome: Outcome
  var progressCount: int

  action init = all {
    outcome' = Running,
    progressCount' = 0,
  }

  // Progress is possible but never forced.
  action progress = all {
    outcome == Running,
    outcome' = Done,
    progressCount' = progressCount + 1,
  }

  // Always enabled: postpones progress forever.
  action skip = all {
    outcome' = outcome,
    progressCount' = progressCount,
  }

  action step = any {
    progress,
    skip,
  }

  // No fairness assumption, so the stall is allowed and liveness fails.
  temporal eventuallyTerminal = eventually(outcome == Done)
}
