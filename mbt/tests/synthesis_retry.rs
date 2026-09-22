//! Model-based test for the client-side retry **arithmetic**.
//!
//! The driver drives the production `RetryPolicy` (with the production
//! `MCP_DAEMON_MAX_RETRIES` constant) and lets Quint Connect compare the
//! resulting attempt/backoff/outcome state against
//! `modeling/quint/SynthesisRetry.qnt` after every step. The attempt *result* is
//! an injected input (`attemptOk` / `attemptFatal` / `attemptRetryable`),
//! matching the model's environment boundary.
//!
//! Scope: this MBT establishes correspondence for the retry decision policy
//! (bound, backoff-vs-stop, counting semantics). It does **not** drive
//! `run_retry_loop` itself; the orchestration (attempt-start counting,
//! cancellation checkpoints, and the wait seam) is covered by the in-file
//! fake-seam tests in `src/interface/mcp_server/tools/text_to_speech.rs`.

use quint_connect::*;
use serde::Deserialize;
use voicevox_cli::domain::synthesis::{AttemptOutcome, RetryDecision, RetryPolicy};
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

struct RetryDriver {
    outcome: Outcome,
    attempts: u32,
    backoffs: u32,
}

impl Default for RetryDriver {
    fn default() -> Self {
        Self {
            outcome: Outcome::Running,
            attempts: 0,
            backoffs: 0,
        }
    }
}

impl RetryDriver {
    fn policy() -> RetryPolicy {
        RetryPolicy::new(MCP_DAEMON_MAX_RETRIES)
    }

    fn reset(&mut self) {
        *self = Self::default();
    }

    fn begin_attempt(&mut self) {
        self.attempts += 1;
        self.outcome = Outcome::Attempting;
    }

    /// Applies the production retry decision for one attempt result.
    fn apply_attempt(&mut self, result: AttemptOutcome) {
        let decision = Self::policy().after_attempt(self.attempts, result);
        self.outcome = match (result, decision) {
            (AttemptOutcome::Succeeded, _) => Outcome::Done,
            (AttemptOutcome::FatalFailure, _) => Outcome::Failed,
            (AttemptOutcome::RetryableFailure, RetryDecision::Backoff) => {
                self.backoffs += 1;
                Outcome::Backoff
            }
            (AttemptOutcome::RetryableFailure, RetryDecision::Finish) => Outcome::Failed,
        };
    }

    fn backoff_done(&mut self) {
        self.outcome = Outcome::Running;
    }

    fn cancel(&mut self) {
        self.outcome = Outcome::Canceled;
    }
}

impl State<RetryDriver> for RetryState {
    fn from_driver(driver: &RetryDriver) -> Result<Self> {
        Ok(Self {
            outcome: driver.outcome.clone(),
            attempts: i64::from(driver.attempts),
            backoffs: i64::from(driver.backoffs),
        })
    }
}

impl Driver for RetryDriver {
    type State = RetryState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            // `init` starts the first trace; quint labels the initial state of
            // subsequent traces with the composite `step` action name (a real
            // step never records `step`), so reset there too.
            init => self.reset(),
            step => self.reset(),
            beginAttempt => self.begin_attempt(),
            attemptOk => self.apply_attempt(AttemptOutcome::Succeeded),
            attemptFatal => self.apply_attempt(AttemptOutcome::FatalFailure),
            attemptRetryable => self.apply_attempt(AttemptOutcome::RetryableFailure),
            backoffDone => self.backoff_done(),
            cancel => self.cancel(),
            _ => (),
        })
    }
}

/// Random exploration of the retry/cancel arithmetic.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    max_samples = 300,
    max_steps = 10
)]
fn synthesis_retry_simulation() -> impl Driver {
    RetryDriver::default()
}
