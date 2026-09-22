//! Model-based test for the **default** MCP streaming synthesis path.
//!
//! `handle_streaming_synthesis` (default `streaming = true`) does not use the
//! retry loop. This driver exercises the production streaming pipeline against
//! a real `voicevox-daemon`:
//!   connect -> listSpeakers -> split -> synthesize each segment ->
//!   concatenate -> (validate WAV)
//! and lets Quint Connect compare the observed lifecycle against
//! `modeling/quint/StreamingSynthesis.qnt`.
//!
//! A 2-segment text is used so the fixed spec scenario is deterministic. Playback
//! is not exercised (it would emit audio); the spec still models it, and the
//! driver stops at concatenation.
//!
//! Requirements:
//! - a built `voicevox-daemon` binary (default `../target/debug/voicevox-daemon`,
//!   or `VOICEVOX_DAEMON_BIN`),
//! - installed VOICEVOX resources (models + ONNX Runtime + OpenJTalk dictionary).
//!
//! This test is `#[ignore]`d by default; run it with `-- --ignored`.

use anyhow::{Context, bail};
use quint_connect::*;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::oneshot;
use voicevox_cli::domain::synthesis::TextSplitter;
use voicevox_cli::domain::synthesis::wav::concatenate_wav_segments;
use voicevox_cli::infrastructure::daemon::client::DaemonClient;
use voicevox_cli::interface::mcp_server::tools::text_to_speech::handle_text_to_speech_cancellable;
use voicevox_cli::interface::mcp_server::tools::types::ToolContent;
use voicevox_cli::interface::synthesis::StreamingSynthesizer;

/// Must split into exactly two non-empty segments via the production splitter.
const SYNTHESIS_TEXT: &str = "いち。に。";
const EXPECTED_SEGMENTS: usize = 2;

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum Phase {
    Idle,
    Connected,
    Segmenting,
    Concatenated,
    Played,
    Failed,
    Canceled,
}

#[cfg(test)]
mod decode_tests {
    use super::{Phase, StreamingState};
    use serde::Deserialize;

    /// Guards the ITF decoding of the spec state without a daemon. Quint encodes
    /// sum-type values and big integers specially (`{"tag":..,"value":..}`,
    /// `{"#tup":..}`, `{"#bigint":..}`); quint-connect decodes them through the
    /// `itf` crate. A missing `#[serde(tag = "tag", content = "value")]` made the
    /// ignored daemon test fail on its first state comparison, so this runs in
    /// the normal test suite.
    #[test]
    fn state_decodes_from_itf_encoding() {
        let raw = serde_json::json!({
            "phase": { "tag": "Connected", "value": { "#tup": [] } },
            "segmentsSynth": { "#bigint": "0" },
            "segmentsTotal": { "#bigint": "0" },
            "playbackStarted": false
        });
        let value: itf::Value = serde_json::from_value(raw).expect("ITF value");
        let state = StreamingState::deserialize(value).expect("spec state must decode");
        assert_eq!(state.phase, Phase::Connected);
        assert_eq!(state.segments_synth, 0);
        assert_eq!(state.segments_total, 0);
        assert!(!state.playback_started);
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct StreamingState {
    phase: Phase,
    #[serde(rename = "segmentsSynth")]
    segments_synth: i64,
    #[serde(rename = "segmentsTotal")]
    segments_total: i64,
    #[serde(rename = "playbackStarted")]
    playback_started: bool,
}

struct StreamingDriver {
    runtime: tokio::runtime::Runtime,
    run_dir: TempDir,
    child: Option<Child>,
    client: Option<DaemonClient>,
    style_id: Option<u32>,
    segments: Option<Vec<Vec<u8>>>,
    phase: Phase,
    segments_synth: i64,
    segments_total: i64,
    playback_started: bool,
}

impl Default for StreamingDriver {
    fn default() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        Self {
            runtime,
            run_dir: secure_tempdir(),
            child: None,
            client: None,
            style_id: None,
            segments: None,
            phase: Phase::Idle,
            segments_synth: 0,
            segments_total: 0,
            playback_started: false,
        }
    }
}

fn secure_tempdir() -> TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
        .expect("chmod 0700");
    dir
}

fn socket_path(driver: &StreamingDriver) -> PathBuf {
    driver.run_dir.path().join("voicevox-daemon.sock")
}

fn daemon_binary() -> PathBuf {
    if let Some(path) = std::env::var_os("VOICEVOX_DAEMON_BIN") {
        return PathBuf::from(path);
    }
    for candidate in [
        "../target/debug/voicevox-daemon",
        "target/debug/voicevox-daemon",
        "../target/release/voicevox-daemon",
        "target/release/voicevox-daemon",
    ] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return path;
        }
    }
    PathBuf::from("voicevox-daemon")
}

fn is_wav(bytes: &[u8]) -> bool {
    bytes.len() > 44 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE"
}

impl StreamingDriver {
    fn spawn_daemon(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }
        let socket = socket_path(self);
        let _ = std::fs::remove_file(&socket);
        let mut command = Command::new(daemon_binary());
        command
            .arg("--foreground")
            .arg("--socket-path")
            .arg(&socket)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if let Some(models_dir) = std::env::var_os("VOICEVOX_MBT_MODELS_DIR") {
            command.env("VOICEVOX_MODELS_DIR", models_dir);
        }
        self.child = Some(command.spawn().context("failed to spawn voicevox-daemon")?);

        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            if socket.exists() {
                return Ok(());
            }
            if let Some(child) = self.child.as_mut() {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        bail!("daemon exited before binding its socket (status: {status})");
                    }
                    Ok(None) => {}
                    Err(error) => bail!("failed to poll the daemon process: {error}"),
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        bail!("daemon did not bind its socket within 300s");
    }

    fn kill_daemon(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn reset(&mut self) {
        self.client = None;
        self.style_id = None;
        self.segments = None;
        self.phase = Phase::Idle;
        self.segments_synth = 0;
        self.segments_total = 0;
        self.playback_started = false;
    }

    fn do_connect(&mut self) -> Result<()> {
        self.spawn_daemon()?;
        let socket = socket_path(self);
        let mut client = self
            .runtime
            .block_on(DaemonClient::new_at(&socket))
            .context("failed to connect to the daemon")?;
        let speakers = self.runtime.block_on(client.list_speakers())?;
        self.style_id = speakers
            .iter()
            .flat_map(|speaker| speaker.styles.iter())
            .map(|style| style.id)
            .min();
        self.client = Some(client);
        self.phase = Phase::Connected;
        self.segments_synth = 0;
        self.segments_total = 0;
        self.playback_started = false;
        Ok(())
    }

    fn do_synthesize_segments(&mut self) -> Result<()> {
        let style_id = self
            .style_id
            .context("no style available: are VOICEVOX models installed?")?;
        let client = self.client.take().context("not connected")?;
        let segmenter = TextSplitter::new(vec!['。'], 100);
        let mut synthesizer =
            StreamingSynthesizer::new_with_client_and_segmenter(client, Box::new(segmenter))?;
        let segments = self
            .runtime
            .block_on(synthesizer.request_streaming_synthesis_segments(
                SYNTHESIS_TEXT,
                style_id,
                1.0,
            ))
            .with_context(|| format!("streaming synthesis failed for style {style_id}"))?;
        if segments.len() != EXPECTED_SEGMENTS {
            bail!(
                "spec scenario expects {EXPECTED_SEGMENTS} segments, production splitter produced {}",
                segments.len()
            );
        }
        for (i, segment) in segments.iter().enumerate() {
            if !is_wav(segment) {
                bail!("segment {i} is not a WAV payload ({} bytes)", segment.len());
            }
        }
        self.segments = Some(segments);
        self.phase = Phase::Segmenting;
        self.segments_synth = i64::try_from(EXPECTED_SEGMENTS).expect("small count");
        self.segments_total = i64::try_from(EXPECTED_SEGMENTS).expect("small count");
        self.playback_started = false;
        Ok(())
    }

    fn do_concatenate(&mut self) -> Result<()> {
        let segments = self.segments.take().context("no segments synthesized")?;
        let wav = concatenate_wav_segments(&segments).context("failed to concatenate segments")?;
        if !is_wav(&wav) {
            bail!("concatenated payload is not a WAV ({} bytes)", wav.len());
        }
        self.phase = Phase::Concatenated;
        Ok(())
    }
}

impl Drop for StreamingDriver {
    fn drop(&mut self) {
        self.kill_daemon();
    }
}

impl State<StreamingDriver> for StreamingState {
    fn from_driver(driver: &StreamingDriver) -> Result<Self> {
        Ok(Self {
            phase: driver.phase.clone(),
            segments_synth: driver.segments_synth,
            segments_total: driver.segments_total,
            playback_started: driver.playback_started,
        })
    }
}

impl Driver for StreamingDriver {
    type State = StreamingState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            scenarioConnect => self.do_connect()?,
            scenarioSynthesizeSegments => self.do_synthesize_segments()?,
            scenarioConcatenate => self.do_concatenate()?,
            scenarioStutter => (),
            _ => (),
        })
    }
}

/// Fixed scenario: connect -> synthesize all segments -> concatenate. This
/// always exercises the real streaming pipeline and WAV assembly.
#[quint_run(
    spec = "../modeling/quint/StreamingSynthesis.qnt",
    init = "scenarioConnect",
    step = "scenarioStep",
    max_samples = 1,
    max_steps = 3
)]
#[ignore = "requires installed VOICEVOX resources; run with -- --ignored"]
fn streaming_synthesis_fixed_scenario() -> impl Driver {
    StreamingDriver::default()
}

/// Daemon-free driver for the streaming failure/cancellation scenarios: points
/// the production `get_socket_path()` at a path with no listener, so the
/// connection used by the default MCP streaming path fails.
struct StreamingFailureDriver {
    runtime: tokio::runtime::Runtime,
    phase: Phase,
    segments_synth: i64,
    segments_total: i64,
    playback_started: bool,
}

impl Default for StreamingFailureDriver {
    fn default() -> Self {
        Self {
            runtime: tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime"),
            phase: Phase::Idle,
            segments_synth: 0,
            segments_total: 0,
            playback_started: false,
        }
    }
}

fn dead_socket_path() -> PathBuf {
    let path = std::env::temp_dir().join("voicevox-mbt-does-not-exist.sock");
    let _ = std::fs::remove_file(&path);
    path
}

impl StreamingFailureDriver {
    fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.segments_synth = 0;
        self.segments_total = 0;
        self.playback_started = false;
    }

    /// Drives the real MCP streaming entry point. `cancel` pre-delivers a
    /// cancellation (otherwise the connect attempt fails against the dead
    /// socket).
    fn run(&mut self, cancel: Option<&str>) {
        // The value is identical for every test in this binary, so parallel
        // tests setting it are benign.
        let path = dead_socket_path();
        unsafe { std::env::set_var("VOICEVOX_SOCKET_PATH", &path) };

        let (cancel_tx, cancel_rx) = oneshot::channel::<String>();
        if let Some(reason) = cancel {
            let _ = cancel_tx.send(reason.to_string());
        }

        let args = json!({
            "text": SYNTHESIS_TEXT,
            "style_id": 3,
            "streaming": true,
        });
        let result = self
            .runtime
            .block_on(handle_text_to_speech_cancellable(args, Some(cancel_rx)));

        self.phase = match result {
            Err(_) => Phase::Failed,
            Ok(tool_result) => {
                let cancelled = tool_result.is_error == Some(true)
                    && tool_result.content.iter().any(|content| match content {
                        ToolContent::Text { text } => text.contains("cancelled"),
                    });
                assert!(
                    cancelled,
                    "streaming connect failure unexpectedly returned a success result"
                );
                Phase::Canceled
            }
        };
        self.segments_synth = 0;
        self.segments_total = 0;
        self.playback_started = false;
    }
}

impl State<StreamingFailureDriver> for StreamingState {
    fn from_driver(driver: &StreamingFailureDriver) -> Result<Self> {
        Ok(Self {
            phase: driver.phase.clone(),
            segments_synth: driver.segments_synth,
            segments_total: driver.segments_total,
            playback_started: driver.playback_started,
        })
    }
}

impl Driver for StreamingFailureDriver {
    type State = StreamingState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            step => self.reset(),
            hold => (),
            scenarioConnectFail => self.run(None),
            scenarioCancelBeforeConnect => self.run(Some("ESC pressed")),
            _ => (),
        })
    }
}

/// A connection failure before any segment is synthesized (daemon-free).
#[quint_run(
    spec = "../modeling/quint/StreamingSynthesis.qnt",
    init = "scenarioConnectFail",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn streaming_connect_failure() -> impl Driver {
    StreamingFailureDriver::default()
}

/// A cancellation delivered before the connection is established (daemon-free).
#[quint_run(
    spec = "../modeling/quint/StreamingSynthesis.qnt",
    init = "scenarioCancelBeforeConnect",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
fn streaming_cancel_before_connect() -> impl Driver {
    StreamingFailureDriver::default()
}
