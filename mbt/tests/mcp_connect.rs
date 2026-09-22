//! Model-based test for the client connect budget.
//!
//! The driver runs the **production** `retry_with_final` (the loop behind
//! `connect_with_retry`) with a counting fake connector and lets Quint Connect
//! compare the resulting connect state and attempt count against the fixed
//! scenarios in `modeling/quint/MCPServer.qnt`.
//!
//! Injected input: how many attempts fail before one succeeds. Observed: the
//! number of attempts made and the terminal client state.

use quint_connect::*;
use serde::Deserialize;
use std::time::Duration;

use voicevox_cli::infrastructure::daemon::client::transport::{ConnectAttempt, retry_with_final};
use voicevox_cli::infrastructure::daemon::startup::MAX_CONNECT_ATTEMPTS;

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum DaemonState {
    DaemonDown,
    DaemonReady,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum ClientState {
    ClientIdle,
    Connecting,
    Connected,
    ConnectFailed,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum PlaybackState {
    PlaybackIdle,
    Launching,
    Playing,
    Stopped,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum PlaybackError {
    NoPlaybackError,
    LaunchFailed,
    DeviceError,
    Canceled,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct ConnectState {
    #[serde(rename = "daemonState")]
    daemon_state: DaemonState,
    #[serde(rename = "clientState")]
    client_state: ClientState,
    attempt: i64,
    #[serde(rename = "playbackAudioReady")]
    playback_audio_ready: bool,
    #[serde(rename = "playbackState")]
    playback_state: PlaybackState,
    #[serde(rename = "playbackCancelRequested")]
    playback_cancel_requested: bool,
    #[serde(rename = "playbackErrorKind")]
    playback_error_kind: PlaybackError,
}

/// Fake connector: fails the first `fail_before_success` attempts, then
/// succeeds, counting every call.
#[derive(Default)]
struct FakeConnector {
    fail_before_success: u32,
    calls: u32,
}

impl ConnectAttempt for FakeConnector {
    type Output = ();
    type Error = ();

    async fn connect_once(&mut self) -> std::result::Result<(), ()> {
        self.calls += 1;
        if self.calls <= self.fail_before_success {
            Err(())
        } else {
            Ok(())
        }
    }
}

struct ConnectDriver {
    runtime: tokio::runtime::Runtime,
    daemon_state: DaemonState,
    client_state: ClientState,
    attempt: i64,
}

impl Default for ConnectDriver {
    fn default() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            daemon_state: DaemonState::DaemonDown,
            client_state: ClientState::ClientIdle,
            attempt: 0,
        }
    }
}

impl ConnectDriver {
    /// Runs the production connect loop, failing `fail_before_success`
    /// attempts first.
    fn run(&mut self, fail_before_success: u32) {
        let mut connector = FakeConnector {
            fail_before_success,
            calls: 0,
        };
        let (result, calls) = self.runtime.block_on(retry_with_final(
            &mut connector,
            MAX_CONNECT_ATTEMPTS,
            Duration::ZERO,
            Duration::ZERO,
        ));

        assert_eq!(connector.calls, calls, "observed calls must match");
        match result {
            Ok(()) => {
                // Connected within the loop: `attempt` counts failed retries.
                self.daemon_state = DaemonState::DaemonReady;
                self.client_state = ClientState::Connected;
                self.attempt = i64::from(calls - 1);
            }
            Err(()) => {
                // The budget is `attempts` loop connects plus one final connect.
                assert_eq!(
                    calls,
                    MAX_CONNECT_ATTEMPTS + 1,
                    "the final connect must add exactly one attempt"
                );
                self.daemon_state = DaemonState::DaemonDown;
                self.client_state = ClientState::ConnectFailed;
                self.attempt = i64::from(MAX_CONNECT_ATTEMPTS);
            }
        }
    }

    fn reset(&mut self) {
        self.daemon_state = DaemonState::DaemonDown;
        self.client_state = ClientState::ClientIdle;
        self.attempt = 0;
    }
}

impl State<ConnectDriver> for ConnectState {
    fn from_driver(driver: &ConnectDriver) -> Result<Self> {
        Ok(Self {
            daemon_state: driver.daemon_state.clone(),
            client_state: driver.client_state.clone(),
            attempt: driver.attempt,
            playback_audio_ready: false,
            playback_state: PlaybackState::PlaybackIdle,
            playback_cancel_requested: false,
            playback_error_kind: PlaybackError::NoPlaybackError,
        })
    }
}

impl Driver for ConnectDriver {
    type State = ConnectState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            step => self.reset(),
            hold => (),
            scenarioConnectedFirst => self.run(0),
            scenarioConnectedAfterRetries => self.run(3),
            scenarioConnectFailed => self.run(MAX_CONNECT_ATTEMPTS + 1),
            _ => (),
        })
    }
}

/// The first connect succeeds.
#[quint_run(
    spec = "../modeling/quint/MCPServer.qnt",
    init = "scenarioConnectedFirst",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_connect_first_attempt() -> impl Driver {
    ConnectDriver::default()
}

/// The connect succeeds after three failed retries.
#[quint_run(
    spec = "../modeling/quint/MCPServer.qnt",
    init = "scenarioConnectedAfterRetries",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_connect_after_retries() -> impl Driver {
    ConnectDriver::default()
}

/// The whole budget (loop attempts plus the final connect) is exhausted.
#[quint_run(
    spec = "../modeling/quint/MCPServer.qnt",
    init = "scenarioConnectFailed",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn mcp_connect_budget_exhausted() -> impl Driver {
    ConnectDriver::default()
}
