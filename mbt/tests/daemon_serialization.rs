//! Model-based test for the daemon's serialized synthesis.
//!
//! Two clients issue concurrent synthesis requests to one real
//! `voicevox-daemon`. The daemon serializes them under a single mutex
//! (`SerializedSynthesisPolicy`); both must complete with valid WAV bytes (no
//! deadlock, no corruption). Quint Connect compares the observed lifecycle
//! against `modeling/quint/DaemonSerialization.qnt`.
//!
//! The daemon worker state is not externally observable, so the driver maps
//! "both concurrent requests completed" to the spec's `scenarioBothDone`
//! state. A regression that serialized incorrectly (deadlock, dropped request,
//! corrupted response) would fail to produce that state.
//!
//! Requirements: a built `voicevox-daemon` binary and installed VOICEVOX
//! resources. This test is `#[ignore]`d by default; run it with `-- --ignored`.

use anyhow::{Context, bail};
use quint_connect::*;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use voicevox_cli::infrastructure::daemon::client::DaemonClient;
use voicevox_cli::infrastructure::ipc::OwnedSynthesizeOptions;

const SYNTHESIS_TEXT: &str = "テスト";

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum WorkerState {
    WorkerIdle,
    Busy,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(tag = "tag", content = "value")]
enum JobState {
    JobWaiting,
    Synthesizing,
    Done,
    Failed,
}

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct SerializationState {
    worker: WorkerState,
    j1: JobState,
    j2: JobState,
}

struct SerializationDriver {
    runtime: tokio::runtime::Runtime,
    run_dir: TempDir,
    child: Option<Child>,
    style_id: Option<u32>,
    worker: WorkerState,
    j1: JobState,
    j2: JobState,
}

impl Default for SerializationDriver {
    fn default() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        Self {
            runtime,
            run_dir: secure_tempdir(),
            child: None,
            style_id: None,
            worker: WorkerState::WorkerIdle,
            j1: JobState::JobWaiting,
            j2: JobState::JobWaiting,
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

fn socket_path(driver: &SerializationDriver) -> PathBuf {
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

impl SerializationDriver {
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
        self.style_id = None;
        self.worker = WorkerState::WorkerIdle;
        self.j1 = JobState::JobWaiting;
        self.j2 = JobState::JobWaiting;
        self.kill_daemon();
    }

    /// Runs two concurrent syntheses and records the observed outcome.
    fn run_concurrent(&mut self) -> Result<()> {
        self.spawn_daemon()?;
        let socket = socket_path(self);

        let mut catalog = self
            .runtime
            .block_on(DaemonClient::new_at(&socket))
            .context("failed to connect first client")?;
        let speakers = self.runtime.block_on(catalog.list_speakers())?;
        let style_id = speakers
            .iter()
            .flat_map(|speaker| speaker.styles.iter())
            .map(|style| style.id)
            .min()
            .context("no style available: are VOICEVOX models installed?")?;

        let mut second = self
            .runtime
            .block_on(DaemonClient::new_at(&socket))
            .context("failed to connect second client")?;

        let (first_result, second_result) = self.runtime.block_on(async {
            let first = async {
                catalog
                    .synthesize(
                        SYNTHESIS_TEXT,
                        style_id,
                        OwnedSynthesizeOptions { rate: 1.0 },
                    )
                    .await
            };
            let other = async {
                second
                    .synthesize(
                        SYNTHESIS_TEXT,
                        style_id,
                        OwnedSynthesizeOptions { rate: 1.0 },
                    )
                    .await
            };
            tokio::join!(first, other)
        });

        let first_wav = first_result.context("first concurrent synthesis failed")?;
        let second_wav = second_result.context("second concurrent synthesis failed")?;
        if !is_wav(&first_wav) || !is_wav(&second_wav) {
            bail!(
                "concurrent syntheses returned non-WAV payloads ({} / {} bytes)",
                first_wav.len(),
                second_wav.len()
            );
        }

        self.style_id = Some(style_id);
        self.worker = WorkerState::WorkerIdle;
        self.j1 = JobState::Done;
        self.j2 = JobState::Done;
        Ok(())
    }
}

impl Drop for SerializationDriver {
    fn drop(&mut self) {
        self.kill_daemon();
    }
}

impl State<SerializationDriver> for SerializationState {
    fn from_driver(driver: &SerializationDriver) -> Result<Self> {
        Ok(Self {
            worker: driver.worker.clone(),
            j1: driver.j1.clone(),
            j2: driver.j2.clone(),
        })
    }
}

impl Driver for SerializationDriver {
    type State = SerializationState;

    #[allow(clippy::no_effect)] // quint-connect's `switch!` macro
    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            hold => (),
            scenarioBothDone => self.run_concurrent()?,
            _ => (),
        })
    }
}

/// Two concurrent syntheses both complete through the serialized daemon.
#[quint_run(
    spec = "../modeling/quint/DaemonSerialization.qnt",
    init = "scenarioBothDone",
    step = "hold",
    max_samples = 1,
    max_steps = 1
)]
#[ignore = "requires installed VOICEVOX resources; run with -- --ignored"]
fn daemon_serialization_concurrent() -> impl Driver {
    SerializationDriver::default()
}
