//! Model-based test for the production retry **state machine**.
//!
//! The driver advances the production `RetryTracker` one spec step at a time
//! (begin attempt / record result / end backoff / cancel) and lets Quint
//! Connect compare `(phase, attempts_started, backoffs_started)` against
//! `modeling/quint/SynthesisRetry.qnt` after every step, over randomly
//! generated traces.
//!
//! Unlike a policy-only check, this exercises the counting semantics
//! (attempts/backoffs counted at start), the terminal rules, and cancellation
//! as they are actually implemented. The async `run_retry_loop` is covered
//! separately by `synthesis_retry_loop.rs`.

use quint_connect::*;
use serde::Deserialize;
use voicevox_cli::domain::synthesis::{AttemptOutcome, RetryPhase, RetryPolicy, RetryTracker};
use voicevox_cli::interface::mcp_server::tools::text_to_speech::MCP_DAEMON_MAX_RETRIES;

/// Mirrors the spec's `Outcome` sum type.
#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Outcome {
    Running,
    Attempting,
    Backoff,
    Done,
    Failed,
    Canceled,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct RetryState {
    outcome: Outcome,
    attempts: i64,
    backoffs: i64,
}

fn outcome_of(phase: RetryPhase) -> Outcome {
    match phase {
        RetryPhase::Running => Outcome::Running,
        RetryPhase::Attempting => Outcome::Attempting,
        RetryPhase::Backoff => Outcome::Backoff,
        RetryPhase::Done => Outcome::Done,
        RetryPhase::Failed => Outcome::Failed,
        RetryPhase::Canceled => Outcome::Canceled,
    }
}

struct TrackerDriver {
    tracker: RetryTracker,
}

impl Default for TrackerDriver {
    fn default() -> Self {
        Self {
            tracker: RetryTracker::new(RetryPolicy::new(MCP_DAEMON_MAX_RETRIES)),
        }
    }
}

impl TrackerDriver {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn record(&mut self, outcome: AttemptOutcome) {
        // `record_attempt` is only enabled from `Attempting` in the spec, so the
        // returned decision is not needed here; the phase/counters are compared
        // through the state.
        let _ = self.tracker.record_attempt(outcome);
    }
}

impl State<TrackerDriver> for RetryState {
    fn from_driver(driver: &TrackerDriver) -> Result<Self> {
        Ok(Self {
            outcome: outcome_of(driver.tracker.phase()),
            attempts: i64::from(driver.tracker.attempts_started()),
            backoffs: i64::from(driver.tracker.backoffs_started()),
        })
    }
}

impl Driver for TrackerDriver {
    type State = RetryState;

    #[allow(clippy::no_effect)] // quint-connect's `switch!` macro
    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            // `init` starts the first trace; `step` is the synthetic first
            // action of subsequent traces (quint labels the initial state with
            // the step relation) and also corresponds to the initial state.
            init => self.reset(),
            step => self.reset(),
            beginAttempt => {
                assert!(
                    self.tracker.begin_attempt(),
                    "spec began an attempt from a non-Running phase"
                );
            }
            attemptOk => self.record(AttemptOutcome::Succeeded),
            attemptFatal => self.record(AttemptOutcome::FatalFailure),
            attemptRetryable => self.record(AttemptOutcome::RetryableFailure),
            backoffDone => {
                assert!(
                    self.tracker.end_backoff(),
                    "spec ended a backoff outside the Backoff phase"
                );
            }
            cancel => {
                assert!(
                    self.tracker.cancel(),
                    "spec cancelled a terminal retry loop"
                );
            }
            _ => (),
        })
    }
}

/// Random exploration of the retry state machine.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    max_samples = 500,
    max_steps = 12
)]
fn synthesis_retry_simulation() -> impl Driver {
    TrackerDriver::default()
}
