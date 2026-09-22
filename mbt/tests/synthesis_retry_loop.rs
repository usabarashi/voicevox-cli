//! Model-based test for the production retry **orchestration**.
//!
//! Unlike `synthesis_retry.rs` (which drives the retry *decision policy*), this
//! driver runs the real `run_retry_loop` with scripted attempt/waiter seams and
//! compares the loop's observed `attempts_started` / `backoffs_started` /
//! terminal outcome against the fixed scenarios in
//! `modeling/quint/SynthesisRetry.qnt`.
//!
//! The attempt results and waits are injected inputs (the environment
//! boundary), but every counter and transition is computed by production code.

use anyhow::anyhow;
use quint_connect::*;
use serde::Deserialize;
use std::collections::VecDeque;
use std::path::Path;
use std::time::Duration;

use voicevox_cli::domain::synthesis::RetryPolicy;
use voicevox_cli::infrastructure::daemon::client::daemon_response_error;
use voicevox_cli::infrastructure::ipc::DaemonErrorCode;
use voicevox_cli::interface::mcp_server::tools::text_to_speech::{
    AttemptCallOutcome, BackoffWaiter, MCP_DAEMON_MAX_RETRIES, RetryLoopResult, SynthesisAttempt,
    WaitOutcome, run_retry_loop,
};
use voicevox_cli::interface::synthesis::flow::DaemonSynthesisBytesRequest;

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

/// Scripted attempt: pops one outcome per production loop invocation.
struct FakeAttempt {
    script: VecDeque<AttemptCallOutcome>,
}

impl SynthesisAttempt for FakeAttempt {
    async fn run(
        &mut self,
        _request: &DaemonSynthesisBytesRequest<'_>,
        _cancel_rx: Option<&mut tokio::sync::oneshot::Receiver<String>>,
    ) -> AttemptCallOutcome {
        self.script
            .pop_front()
            .unwrap_or_else(|| AttemptCallOutcome::Failed(anyhow!("unexpected attempt")))
    }
}

/// Scripted backoff waiter: pops one outcome per backoff.
struct FakeWaiter {
    script: VecDeque<WaitOutcome>,
}

impl BackoffWaiter for FakeWaiter {
    async fn wait(
        &mut self,
        _delay: Duration,
        _cancel_rx: Option<&mut tokio::sync::oneshot::Receiver<String>>,
    ) -> WaitOutcome {
        self.script.pop_front().unwrap_or(WaitOutcome::Elapsed)
    }
}

fn retryable_error() -> anyhow::Error {
    daemon_response_error("ctx", DaemonErrorCode::SynthesisFailed, "temporary failure")
}

fn fatal_error() -> anyhow::Error {
    daemon_response_error("ctx", DaemonErrorCode::InvalidTargetId, "bad id")
}

struct RetryLoopDriver {
    runtime: tokio::runtime::Runtime,
    outcome: Option<Outcome>,
    attempts: u32,
    backoffs: u32,
}

impl Default for RetryLoopDriver {
    fn default() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            outcome: None,
            attempts: 0,
            backoffs: 0,
        }
    }
}

impl RetryLoopDriver {
    fn reset(&mut self) {
        self.outcome = Some(Outcome::Running);
        self.attempts = 0;
        self.backoffs = 0;
    }

    /// Records a state observed without running the loop (used for the
    /// pre-cancelled scenario, where no attempt may start).
    fn record(&mut self, outcome: Outcome, attempts: u32, backoffs: u32) {
        self.outcome = Some(outcome);
        self.attempts = attempts;
        self.backoffs = backoffs;
    }

    /// Runs the loop with a cancellation that is already delivered before the
    /// first attempt.
    fn run_loop_with_precancel(&mut self, reason: &str) {
        let (tx, rx) = tokio::sync::oneshot::channel::<String>();
        let _ = tx.send(reason.to_string());
        let mut cancel_rx = Some(rx);
        let mut attempt = FakeAttempt {
            script: VecDeque::new(),
        };
        let mut waiter = FakeWaiter {
            script: VecDeque::new(),
        };
        let socket = Path::new("/tmp/does-not-exist-mbt.sock");
        let request = DaemonSynthesisBytesRequest {
            text: "test",
            style_id: 3,
            rate: 1.0,
            socket_path: socket,
            ensure_models_if_missing: false,
            quiet_setup_messages: true,
        };
        let result = self.runtime.block_on(run_retry_loop(
            RetryPolicy::new(MCP_DAEMON_MAX_RETRIES),
            &mut attempt,
            &mut waiter,
            &request,
            &mut cancel_rx,
        ));
        self.record(
            Outcome::Canceled,
            result.attempts_started,
            result.backoffs_started,
        );
    }

    /// Runs the production `run_retry_loop` with the given injected inputs and
    /// records the loop's observed result.
    fn run_loop(&mut self, attempts: Vec<AttemptCallOutcome>, waits: Vec<WaitOutcome>) {
        let mut attempt = FakeAttempt {
            script: attempts.into(),
        };
        let mut waiter = FakeWaiter {
            script: waits.into(),
        };
        let socket = Path::new("/tmp/does-not-exist-mbt.sock");
        let request = DaemonSynthesisBytesRequest {
            text: "test",
            style_id: 3,
            rate: 1.0,
            socket_path: socket,
            ensure_models_if_missing: false,
            quiet_setup_messages: true,
        };
        let mut cancel_rx = None;

        let result: RetryLoopResult = self.runtime.block_on(run_retry_loop(
            RetryPolicy::new(MCP_DAEMON_MAX_RETRIES),
            &mut attempt,
            &mut waiter,
            &request,
            &mut cancel_rx,
        ));

        self.outcome = Some(if result.cancellation.is_some() {
            Outcome::Canceled
        } else if result.wav_data.is_some() {
            Outcome::Done
        } else {
            Outcome::Failed
        });
        self.attempts = result.attempts_started;
        self.backoffs = result.backoffs_started;
    }
}

impl State<RetryLoopDriver> for RetryState {
    fn from_driver(driver: &RetryLoopDriver) -> Result<Self> {
        Ok(Self {
            outcome: driver.outcome.clone().expect("outcome not set"),
            attempts: i64::from(driver.attempts),
            backoffs: i64::from(driver.backoffs),
        })
    }
}

impl Driver for RetryLoopDriver {
    type State = RetryState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            // Mirrors the spec's `init`. `step` is the synthetic first action
            // of traces after the first (quint labels the initial state with the
            // step relation) and also corresponds to the initial state.
            init => self.reset(),
            step => self.reset(),

            scenarioExhaustRetryable => self.run_loop(
                vec![
                    AttemptCallOutcome::Failed(retryable_error()),
                    AttemptCallOutcome::Failed(retryable_error()),
                    AttemptCallOutcome::Failed(retryable_error()),
                ],
                vec![WaitOutcome::Elapsed, WaitOutcome::Elapsed],
            ),
            scenarioSuccessAfterRetry => self.run_loop(
                vec![
                    AttemptCallOutcome::Failed(retryable_error()),
                    AttemptCallOutcome::Completed(vec![1, 2, 3]),
                ],
                vec![WaitOutcome::Elapsed],
            ),
            scenarioFatalFirst => self.run_loop(
                vec![AttemptCallOutcome::Failed(fatal_error())],
                vec![],
            ),
            scenarioCancelBeforeFirstAttempt => {
                self.run_loop_with_precancel("ESC pressed");
            }
            scenarioCancelDuringBackoff => self.run_loop(
                vec![AttemptCallOutcome::Failed(retryable_error())],
                vec![WaitOutcome::Cancelled("ESC pressed".to_string())],
            ),
            scenarioCancelInFlightThird => self.run_loop(
                vec![
                    AttemptCallOutcome::Failed(retryable_error()),
                    AttemptCallOutcome::Failed(retryable_error()),
                    AttemptCallOutcome::Cancelled("ESC pressed".to_string()),
                ],
                vec![WaitOutcome::Elapsed, WaitOutcome::Elapsed],
            ),
            hold => (),
            _ => (),
        })
    }
}

/// Three retryable failures exhaust the attempts and start two backoffs.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    init = "scenarioExhaustRetryable",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn retry_loop_exhausts_retryable_failures() -> impl Driver {
    RetryLoopDriver::default()
}

/// A retryable failure followed by success.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    init = "scenarioSuccessAfterRetry",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn retry_loop_succeeds_after_retry() -> impl Driver {
    RetryLoopDriver::default()
}

/// A non-retryable failure stops on the first attempt.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    init = "scenarioFatalFirst",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn retry_loop_stops_on_fatal_error() -> impl Driver {
    RetryLoopDriver::default()
}

/// Cancellation during a backoff prevents the next attempt.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    init = "scenarioCancelDuringBackoff",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn retry_loop_cancel_during_backoff() -> impl Driver {
    RetryLoopDriver::default()
}

/// Cancellation already delivered before the first attempt starts no attempt.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    init = "scenarioCancelBeforeFirstAttempt",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn retry_loop_cancel_before_first_attempt() -> impl Driver {
    RetryLoopDriver::default()
}

/// Cancellation during the third in-flight attempt is observed as `Canceled`
/// with three attempts (and two backoffs) started.
#[quint_run(
    spec = "../modeling/quint/SynthesisRetry.qnt",
    init = "scenarioCancelInFlightThird",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn retry_loop_cancel_in_flight_third_attempt() -> impl Driver {
    RetryLoopDriver::default()
}
