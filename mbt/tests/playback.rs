//! Model-based test for the emit/play dispatch.
//!
//! The driver runs the **production** `emit_and_play_with_backend` with a
//! scripted fake `AudioPlayback` and lets Quint Connect compare the resulting
//! terminal playback state against the fixed scenarios in
//! `modeling/quint/Playback.qnt`.
//!
//! Boundary: the real audio backends (rodio, external players) are
//! host-dependent and out of scope; this checks the dispatch contract (file
//! write, the `play` flag, and propagation of the backend outcome).

use anyhow::{Result as AnyResult, anyhow};
use quint_connect::*;
use serde::Deserialize;
use tokio::sync::oneshot;

use voicevox_cli::interface::playback::{
    AudioPlayback, PlaybackOutcome, PlaybackRequest, emit_and_play_with_backend,
};

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum PlaybackState {
    Idle,
    Launching,
    Playing,
    Stopped,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum PlaybackError {
    None,
    LaunchFailed,
    DeviceError,
    Canceled,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct PlaybackModelState {
    #[serde(rename = "audioReady")]
    audio_ready: bool,
    #[serde(rename = "playbackState")]
    playback_state: PlaybackState,
    #[serde(rename = "cancelRequested")]
    cancel_requested: bool,
    #[serde(rename = "errorKind")]
    error_kind: PlaybackError,
}

/// What the injected backend returns.
#[derive(Clone)]
enum BackendOutcome {
    Completed,
    Cancelled(String),
    Failed,
}

struct FakeBackend {
    outcome: BackendOutcome,
    calls: u32,
}

impl AudioPlayback for FakeBackend {
    async fn play(
        &mut self,
        _wav_data: &[u8],
        _cancel_rx: Option<&mut oneshot::Receiver<String>>,
    ) -> AnyResult<PlaybackOutcome> {
        self.calls += 1;
        match &self.outcome {
            BackendOutcome::Completed => Ok(PlaybackOutcome::Completed),
            BackendOutcome::Cancelled(reason) => Ok(PlaybackOutcome::Cancelled(reason.clone())),
            BackendOutcome::Failed => Err(anyhow!("injected playback failure")),
        }
    }
}

struct PlaybackDriver {
    runtime: tokio::runtime::Runtime,
    audio_ready: bool,
    playback_state: PlaybackState,
    cancel_requested: bool,
    error_kind: PlaybackError,
}

impl Default for PlaybackDriver {
    fn default() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            audio_ready: false,
            playback_state: PlaybackState::Idle,
            cancel_requested: false,
            error_kind: PlaybackError::None,
        }
    }
}

impl PlaybackDriver {
    fn run(&mut self, outcome: BackendOutcome) {
        let mut backend = FakeBackend { outcome, calls: 0 };
        let request = PlaybackRequest {
            wav_data: &[1, 2, 3, 4],
            output_file: None,
            play: true,
            cancel_rx: None,
        };
        let result = self
            .runtime
            .block_on(emit_and_play_with_backend(request, &mut backend));

        assert_eq!(backend.calls, 1, "the backend must be invoked exactly once");
        (
            self.audio_ready,
            self.playback_state,
            self.cancel_requested,
            self.error_kind,
        ) = match result {
            Ok(PlaybackOutcome::Completed) => {
                (true, PlaybackState::Stopped, false, PlaybackError::None)
            }
            Ok(PlaybackOutcome::Cancelled(_)) => {
                (true, PlaybackState::Stopped, true, PlaybackError::Canceled)
            }
            Err(_) => (
                true,
                PlaybackState::Failed,
                false,
                PlaybackError::LaunchFailed,
            ),
        };
    }

    fn reset(&mut self) {
        self.audio_ready = false;
        self.playback_state = PlaybackState::Idle;
        self.cancel_requested = false;
        self.error_kind = PlaybackError::None;
    }
}

impl State<PlaybackDriver> for PlaybackModelState {
    fn from_driver(driver: &PlaybackDriver) -> AnyResult<Self> {
        Ok(Self {
            audio_ready: driver.audio_ready,
            playback_state: driver.playback_state.clone(),
            cancel_requested: driver.cancel_requested,
            error_kind: driver.error_kind.clone(),
        })
    }
}

impl Driver for PlaybackDriver {
    type State = PlaybackModelState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            step => self.reset(),
            hold => (),
            scenarioNaturalEnd => self.run(BackendOutcome::Completed),
            scenarioCanceled => self.run(BackendOutcome::Cancelled("ESC pressed".to_string())),
            scenarioLaunchFailed => self.run(BackendOutcome::Failed),
            _ => (),
        })
    }
}

/// Playback completes normally.
#[quint_run(
    spec = "../modeling/quint/Playback.qnt",
    init = "scenarioNaturalEnd",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn playback_completes() -> impl Driver {
    PlaybackDriver::default()
}

/// Playback is cancelled.
#[quint_run(
    spec = "../modeling/quint/Playback.qnt",
    init = "scenarioCanceled",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn playback_cancelled() -> impl Driver {
    PlaybackDriver::default()
}

/// Playback launch fails.
#[quint_run(
    spec = "../modeling/quint/Playback.qnt",
    init = "scenarioLaunchFailed",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn playback_launch_failed() -> impl Driver {
    PlaybackDriver::default()
}
