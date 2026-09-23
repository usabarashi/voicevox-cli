//! Model-based test for the real daemon IPC path.
//!
//! The driver spawns a real `voicevox-daemon`, connects to it over the Unix
//! socket, and reads the catalog. Quint Connect compares the observed
//! connection/catalog-read lifecycle against `modeling/quint/DaemonIpc.qnt`.
//!
//! Catalog *contents* are environment dependent (empty without downloaded
//! models, populated with them), so the spec tracks only whether a catalog was
//! read; catalog integrity is asserted here.
//!
//! Requirements:
//! - a built `voicevox-daemon` binary (default `../target/debug/voicevox-daemon`,
//!   or `VOICEVOX_DAEMON_BIN`),
//! - optionally `VOICEVOX_MBT_MODELS_DIR` pointing at real models; without it an
//!   empty temporary models directory is used.
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

#[derive(Clone, PartialEq, Eq, Debug, Deserialize)]
struct DaemonIpcState {
    connected: bool,
    #[serde(rename = "catalogRead")]
    catalog_read: bool,
}

struct DaemonIpcDriver {
    runtime: tokio::runtime::Runtime,
    run_dir: TempDir,
    models_dir: Option<TempDir>,
    child: Option<Child>,
    client: Option<DaemonClient>,
    connected: bool,
    catalog_read: bool,
}

impl Default for DaemonIpcDriver {
    fn default() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        let run_dir = secure_tempdir();
        let models_dir = if std::env::var_os("VOICEVOX_MBT_MODELS_DIR").is_some() {
            None
        } else {
            Some(secure_tempdir())
        };
        Self {
            runtime,
            run_dir,
            models_dir,
            child: None,
            client: None,
            connected: false,
            catalog_read: false,
        }
    }
}

fn models_dir_path(driver: &DaemonIpcDriver) -> PathBuf {
    std::env::var_os("VOICEVOX_MBT_MODELS_DIR").map_or_else(
        || {
            driver
                .models_dir
                .as_ref()
                .expect("temp models dir")
                .path()
                .to_path_buf()
        },
        PathBuf::from,
    )
}

fn socket_path(driver: &DaemonIpcDriver) -> PathBuf {
    driver.run_dir.path().join("voicevox-daemon.sock")
}

/// Creates a temporary directory with mode 0700. The daemon refuses a socket
/// whose parent directory is group/world accessible.
fn secure_tempdir() -> TempDir {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("temp dir");
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
        .expect("chmod 0700");
    dir
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

impl DaemonIpcDriver {
    fn spawn_daemon(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }
        let socket = socket_path(self);
        let _ = std::fs::remove_file(&socket);
        let child = Command::new(daemon_binary())
            .arg("--foreground")
            .arg("--socket-path")
            .arg(&socket)
            .env("VOICEVOX_MODELS_DIR", models_dir_path(self))
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .context("failed to spawn voicevox-daemon")?;
        self.child = Some(child);

        let deadline = Instant::now() + Duration::from_secs(30);
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
            std::thread::sleep(Duration::from_millis(100));
        }
        bail!("daemon did not bind its socket within 30s");
    }

    fn kill_daemon(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
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
        Ok(())
    }

    fn do_list_speakers(&mut self) -> Result<()> {
        let client = self.client.as_mut().context("not connected")?;
        let speakers = self.runtime.block_on(client.list_speakers())?;
        // Catalog integrity: a speaker with no name is malformed.
        for speaker in &speakers {
            if speaker.name.is_empty() {
                bail!("daemon returned a speaker with an empty name");
            }
        }
        self.catalog_read = true;
        Ok(())
    }

    fn do_list_models(&mut self) -> Result<()> {
        let client = self.client.as_mut().context("not connected")?;
        self.runtime.block_on(client.list_models())?;
        self.catalog_read = true;
        Ok(())
    }

    fn reset(&mut self) {
        self.client = None;
        self.kill_daemon();
        self.connected = false;
        self.catalog_read = false;
    }
}

impl Drop for DaemonIpcDriver {
    fn drop(&mut self) {
        self.kill_daemon();
    }
}

impl State<DaemonIpcDriver> for DaemonIpcState {
    fn from_driver(driver: &DaemonIpcDriver) -> Result<Self> {
        Ok(Self {
            connected: driver.connected,
            catalog_read: driver.catalog_read,
        })
    }
}

impl Driver for DaemonIpcDriver {
    type State = DaemonIpcState;

    #[allow(clippy::no_effect)] // quint-connect's `switch!` macro
    fn step(&mut self, step: &Step) -> Result {
        switch!(step {
            // `init` is the first state of every trace; reset fully so the
            // driver is reusable across traces.
            init => self.reset(),
            connect => self.do_connect()?,
            listSpeakers => self.do_list_speakers()?,
            listModels => self.do_list_models()?,
            disconnect => self.reset(),
            // Fixed scenario actions.
            scenarioConnect => self.do_connect()?,
            scenarioList => self.do_list_speakers()?,
            scenarioStutter => (),
            _ => (),
        })
    }
}

/// Real-daemon connect/list lifecycle, compared against `DaemonIpc.qnt`.
#[quint_run(
    spec = "../modeling/quint/DaemonIpc.qnt",
    max_samples = 3,
    max_steps = 6
)]
#[ignore = "spawns a real voicevox-daemon; run with -- --ignored"]
fn daemon_ipc_simulation() -> impl Driver {
    DaemonIpcDriver::default()
}

/// Fixed scenario: connect then read the catalog. This always exercises a real
/// list request, unlike the random simulation which may skip it.
#[quint_run(
    spec = "../modeling/quint/DaemonIpc.qnt",
    init = "scenarioConnect",
    step = "scenarioStep",
    max_samples = 1,
    max_steps = 3
)]
#[ignore = "spawns a real voicevox-daemon; run with -- --ignored"]
fn daemon_ipc_fixed_scenario() -> impl Driver {
    DaemonIpcDriver::default()
}
