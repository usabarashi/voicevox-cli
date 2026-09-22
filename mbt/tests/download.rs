//! Model-based test for the download installer retry loop.
//!
//! The driver runs the **production** `install_with_retries` (driven by
//! `DownloadTracker`) with a scripted fake installer and lets Quint Connect
//! compare the resulting `(state, attempts, failure)` against the fixed
//! scenarios in `modeling/quint/Download.qnt`.
//!
//! Injected inputs: the downloader outcome sequence (environment). Observed:
//! the number of invocations, the terminal state, and the failure class.

use quint_connect::*;
use serde::Deserialize;
use std::collections::VecDeque;

use voicevox_cli::infrastructure::download::{
    DownloadAttempt, DownloadFailure, DownloadPhase, MAX_DOWNLOAD_ATTEMPTS, ResourceInstaller,
    install_with_retries,
};

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum DState {
    Idle,
    Downloading,
    Cleaning,
    Done,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum FailureKind {
    NoFailure,
    PreparationFailed,
    Exhausted,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct DownloadState {
    state: DState,
    attempts: i64,
    failure: FailureKind,
}

/// Fake installer: pops one scripted outcome per invocation, and records the
/// cleanup/retry calls the loop makes.
#[derive(Default)]
struct FakeInstaller {
    script: VecDeque<DownloadAttempt>,
    run_calls: u32,
    cleanup_calls: u32,
}

impl FakeInstaller {
    fn new(script: Vec<DownloadAttempt>) -> Self {
        Self {
            script: script.into(),
            run_calls: 0,
            cleanup_calls: 0,
        }
    }
}

impl ResourceInstaller for FakeInstaller {
    async fn run_once(&mut self) -> DownloadAttempt {
        self.run_calls += 1;
        self.script
            .pop_front()
            .unwrap_or(DownloadAttempt::Succeeded)
    }

    fn cleanup(&mut self) {
        self.cleanup_calls += 1;
    }

    async fn wait_before_retry(&mut self) {}
}

fn failed() -> DownloadAttempt {
    DownloadAttempt::Failed {
        detail: "injected failure".to_string(),
    }
}

struct DownloadDriver {
    runtime: tokio::runtime::Runtime,
    state: DState,
    attempts: i64,
    failure: FailureKind,
}

impl Default for DownloadDriver {
    fn default() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            state: DState::Idle,
            attempts: 0,
            failure: FailureKind::NoFailure,
        }
    }
}

impl DownloadDriver {
    fn run(&mut self, script: Vec<DownloadAttempt>) {
        let mut installer = FakeInstaller::new(script);
        let report = self
            .runtime
            .block_on(install_with_retries(&mut installer, MAX_DOWNLOAD_ATTEMPTS));

        self.state = match report.phase {
            DownloadPhase::Idle => DState::Idle,
            DownloadPhase::Downloading => DState::Downloading,
            DownloadPhase::Cleaning => DState::Cleaning,
            DownloadPhase::Done => DState::Done,
            DownloadPhase::Failed => DState::Failed,
        };
        self.attempts = i64::from(report.attempts);
        self.failure = match report.failure {
            DownloadFailure::NoFailure => FailureKind::NoFailure,
            DownloadFailure::Exhausted => FailureKind::Exhausted,
        };
        assert_eq!(
            installer.run_calls, report.attempts,
            "observed attempts must match installer invocations"
        );
    }

    fn reset(&mut self) {
        self.state = DState::Idle;
        self.attempts = 0;
        self.failure = FailureKind::NoFailure;
    }
}

impl State<DownloadDriver> for DownloadState {
    fn from_driver(driver: &DownloadDriver) -> Result<Self> {
        Ok(Self {
            state: driver.state.clone(),
            attempts: driver.attempts,
            failure: driver.failure.clone(),
        })
    }
}

impl Driver for DownloadDriver {
    type State = DownloadState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            step => self.reset(),
            hold => (),
            scenarioDoneFirst => self.run(vec![DownloadAttempt::Succeeded]),
            scenarioDoneThird => self.run(vec![failed(), failed(), DownloadAttempt::Succeeded]),
            scenarioExhausted => self.run(vec![failed(), failed(), failed()]),
            _ => (),
        })
    }
}

/// The first invocation succeeds.
#[quint_run(
    spec = "../modeling/quint/Download.qnt",
    init = "scenarioDoneFirst",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn download_succeeds_first_attempt() -> impl Driver {
    DownloadDriver::default()
}

/// Two failures, then success on the third invocation.
#[quint_run(
    spec = "../modeling/quint/Download.qnt",
    init = "scenarioDoneThird",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn download_succeeds_third_attempt() -> impl Driver {
    DownloadDriver::default()
}

/// Three failures exhaust the invocation budget.
#[quint_run(
    spec = "../modeling/quint/Download.qnt",
    init = "scenarioExhausted",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn download_exhausts_attempts() -> impl Driver {
    DownloadDriver::default()
}
