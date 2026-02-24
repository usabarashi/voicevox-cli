use anyhow::{anyhow, Result};
use std::path::Path;
use std::time::Duration;
use tokio::net::UnixStream;

use super::transport::{connect_socket_with_timeout, DAEMON_CONNECTION_TIMEOUT};

const DAEMON_STARTUP_MAX_RETRIES: u32 = 20;
const DAEMON_STARTUP_INITIAL_DELAY: Duration = Duration::from_millis(500);
const DAEMON_STARTUP_MAX_DELAY: Duration = Duration::from_secs(4);
const DAEMON_STARTUP_GRACE_PERIOD: Duration = Duration::from_millis(1000);
const DAEMON_FINAL_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
const DAEMON_STARTUP_TOTAL_TIME_ESTIMATE: u32 = 80;

async fn wait_for_daemon_startup(socket_path: &Path) -> Result<()> {
    let ready = crate::daemon::socket_probe::wait_for_socket_ready_with_backoff(
        socket_path,
        DAEMON_STARTUP_MAX_RETRIES,
        DAEMON_STARTUP_INITIAL_DELAY,
        DAEMON_STARTUP_MAX_DELAY,
        false,
        |_| {
            use std::io::Write as _;
            print!(".");
            let _ = std::io::stdout().flush();
        },
    )
    .await;

    ready.then_some(()).ok_or_else(|| {
        anyhow!(
            "Daemon not responding after {DAEMON_STARTUP_MAX_RETRIES} attempts (~{DAEMON_STARTUP_TOTAL_TIME_ESTIMATE}s total)"
        )
    })
}

async fn start_daemon_automatically(socket_path: &Path) -> Result<()> {
    use std::io::Write;

    println!("Starting VOICEVOX daemon (first startup may take a few seconds)...");
    println!("  Resources:");
    println!(
        "    ONNX Runtime: {}",
        crate::paths::find_onnxruntime()?.display()
    );
    println!(
        "    OpenJTalk Dictionary: {}",
        crate::paths::find_openjtalk_dict()?.display()
    );

    let models_dir = crate::paths::find_models_dir()?;
    let models = crate::voice::scan_available_models()?;
    println!(
        "    Voice Models: {} in {}",
        models.len(),
        models_dir.display()
    );
    println!("  Building voice model mappings (this may take a moment)...");

    print!("  Starting daemon process");
    std::io::stdout().flush()?;

    match crate::daemon::start_daemon_detached(Some(socket_path)).await {
        Ok(crate::daemon::StartDaemonOutcome::Started) => {
            wait_for_daemon_startup(socket_path).await?;
            println!(" done!");
            println!("VOICEVOX daemon started successfully");
            Ok(())
        }
        Ok(crate::daemon::StartDaemonOutcome::AlreadyRunning) => {
            wait_for_daemon_startup(socket_path).await?;
            println!(" done!");
            println!("VOICEVOX daemon is already running");
            Ok(())
        }
        Err(error) => Err(anyhow!("Failed to execute daemon: {error}")),
    }
}

pub(crate) async fn connect_or_start(socket_path: &Path) -> Result<UnixStream> {
    if let Ok(stream) = connect_socket_with_timeout(socket_path, DAEMON_CONNECTION_TIMEOUT).await {
        return Ok(stream);
    }

    crate::voice::has_available_models().then_some(()).ok_or_else(|| {
        anyhow!(
            "No VOICEVOX models found. Please run 'voicevox-setup' or place .vvm files in the models directory."
        )
    })?;

    start_daemon_automatically(socket_path).await?;
    tokio::time::sleep(DAEMON_STARTUP_GRACE_PERIOD).await;

    connect_socket_with_timeout(socket_path, DAEMON_FINAL_CONNECTION_TIMEOUT)
        .await
        .map_err(|e| {
            anyhow!(
                "Daemon started but failed to connect at {}: {e}",
                socket_path.display()
            )
        })
}
