//! Model-based test for real daemon synthesis (Phase 3 (B)).
//!
//! The driver spawns a real `voicevox-daemon`, connects, reads the catalog to
//! pick a valid style, requests synthesis, and validates the returned WAV.
//! Quint Connect compares the observed lifecycle against
//! `modeling/quint/DaemonSynthesize.qnt`.
//!
//! Requirements:
//! - a built `voicevox-daemon` binary,
//! - installed VOICEVOX resources (models + ONNX Runtime + OpenJTalk dictionary),
//!   normally provisioned with `voicevox-setup`. Without them the catalog is
//!   empty and the test cannot pick a style; it fails loudly.
//!
//! This test is `#[ignore]`d by default; run it with `-- --ignored`.

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
struct DaemonSynthesizeState {
    connected: bool,
    #[serde(rename = "catalogRead")]
    catalog_read: bool,
    synthesized: bool,
}

struct DaemonSynthesizeDriver {
    runtime: tokio::runtime::Runtime,
    run_dir: TempDir,
    child: Option<Child>,
    client: Option<DaemonClient>,
    style_id: Option<u32>,
    connected: bool,
    catalog_read: bool,
    synthesized: bool,
}

impl Default for DaemonSynthesizeDriver {
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
            connected: false,
            catalog_read: false,
            synthesized: false,
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

fn socket_path(driver: &DaemonSynthesizeDriver) -> PathBuf {
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

impl DaemonSynthesizeDriver {
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

        // The daemon builds its catalog (loading every model once) before
        // binding its socket; with a populated models directory this can take
        // minutes on a cold CI runner.
        let deadline = Instant::now() + Duration::from_secs(300);
        while Instant::now() < deadline {
            if socket.exists() {
                return Ok(());
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
        // Keep the daemon process alive across traces: rebuilding its catalog
        // (loading every model once) is the expensive part. The process is
        // killed in `Drop`.
        self.client = None;
        self.style_id = None;
        self.connected = false;
        self.catalog_read = false;
        self.synthesized = false;
    }

    fn do_connect(&mut self) -> Result<()> {
        self.spawn_daemon()?;
        let socket = socket_path(self);
        let client = self
            .runtime
            .block_on(DaemonClient::new_at(&socket))
            .context("failed to connect to the daemon")?;
        self.client = Some(client);
        self.connected = true;
        self.catalog_read = false;
        self.synthesized = false;
        Ok(())
    }

    fn do_list_speakers(&mut self) -> Result<()> {
        let client = self.client.as_mut().context("not connected")?;
        let speakers = self.runtime.block_on(client.list_speakers())?;
        self.style_id = speakers
            .iter()
            .flat_map(|speaker| speaker.styles.iter())
            .map(|style| style.id)
            .min();
        self.catalog_read = true;
        Ok(())
    }

    fn do_synthesize(&mut self) -> Result<()> {
        let style_id = self
            .style_id
            .context("no style available: are VOICEVOX models installed?")?;
        let client = self.client.as_mut().context("not connected")?;
        let wav = self
            .runtime
            .block_on(client.synthesize(
                SYNTHESIS_TEXT,
                style_id,
                OwnedSynthesizeOptions { rate: 1.0 },
            ))
            .with_context(|| format!("synthesis failed for style {style_id}"))?;
        if !is_wav(&wav) {
            bail!("daemon returned a non-WAV payload ({} bytes)", wav.len());
        }
        self.synthesized = true;
        Ok(())
    }
}

impl Drop for DaemonSynthesizeDriver {
    fn drop(&mut self) {
        self.kill_daemon();
    }
}

impl State<DaemonSynthesizeDriver> for DaemonSynthesizeState {
    fn from_driver(driver: &DaemonSynthesizeDriver) -> Result<Self> {
        Ok(Self {
            connected: driver.connected,
            catalog_read: driver.catalog_read,
            synthesized: driver.synthesized,
        })
    }
}

impl Driver for DaemonSynthesizeDriver {
    type State = DaemonSynthesizeState;

    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            init => self.reset(),
            connect => self.do_connect()?,
            listSpeakers => self.do_list_speakers()?,
            synthesize => self.do_synthesize()?,
            disconnect => self.reset(),
            // Fixed scenario actions.
            scenarioConnect => self.do_connect()?,
            scenarioList => self.do_list_speakers()?,
            scenarioSynthesize => self.do_synthesize()?,
            scenarioStutter => (),
            _ => (),
        })
    }
}

/// Real-daemon synthesis lifecycle, compared against `DaemonSynthesize.qnt`.
#[quint_run(
    spec = "../modeling/quint/DaemonSynthesize.qnt",
    max_samples = 3,
    max_steps = 6
)]
#[ignore = "requires installed VOICEVOX resources; run with -- --ignored"]
fn daemon_synthesize_simulation() -> impl Driver {
    DaemonSynthesizeDriver::default()
}

/// Fixed scenario: connect -> listSpeakers -> synthesize. This always exercises
/// a real synthesis, unlike the random simulation which may skip it.
#[quint_run(
    spec = "../modeling/quint/DaemonSynthesize.qnt",
    init = "scenarioConnect",
    step = "scenarioStep",
    max_samples = 1,
    max_steps = 4
)]
#[ignore = "requires installed VOICEVOX resources; run with -- --ignored"]
fn daemon_synthesize_fixed_scenario() -> impl Driver {
    DaemonSynthesizeDriver::default()
}
