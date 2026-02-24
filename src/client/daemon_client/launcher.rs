use anyhow::{anyhow, Result};
use std::path::Path;
use std::time::Duration;
use tokio::net::UnixStream;

use super::transport::{connect_socket_with_timeout, DAEMON_CONNECTION_TIMEOUT};
use crate::daemon::{
    ensure_daemon_running, EnsureDaemonRunningOptions, EnsureDaemonRunningOutcome,
};

const DAEMON_STARTUP_GRACE_PERIOD: Duration = Duration::from_millis(1000);
const DAEMON_FINAL_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
const DAEMON_STARTUP_MAX_RETRIES: u32 = 20;
const DAEMON_STARTUP_INITIAL_DELAY: Duration = Duration::from_millis(500);
const DAEMON_STARTUP_MAX_DELAY: Duration = Duration::from_secs(4);

async fn start_daemon_automatically(socket_path: &Path) -> Result<()> {
    use std::io::Write;

    crate::logging::info("Starting VOICEVOX daemon (first startup may take a few seconds)...");
    crate::logging::info("  Resources:");
    crate::logging::info(&format!(
        "    ONNX Runtime: {}",
        crate::paths::find_onnxruntime()?.display()
    ));
    crate::logging::info(&format!(
        "    OpenJTalk Dictionary: {}",
        crate::paths::find_openjtalk_dict()?.display()
    ));

    let models_dir = crate::paths::find_models_dir()?;
    let models = crate::voice::scan_available_models()?;
    crate::logging::info(&format!(
        "    Voice Models: {} in {}",
        models.len(),
        models_dir.display()
    ));
    crate::logging::info("  Building voice model mappings (this may take a moment)...");

    print!("  Starting daemon process");
    std::io::stdout().flush()?;

    let startup_options = EnsureDaemonRunningOptions {
        connect_timeout: DAEMON_CONNECTION_TIMEOUT,
        wait_attempts: DAEMON_STARTUP_MAX_RETRIES,
        initial_retry_delay: DAEMON_STARTUP_INITIAL_DELAY,
        max_retry_delay: DAEMON_STARTUP_MAX_DELAY,
        ..EnsureDaemonRunningOptions::default()
    };

    match ensure_daemon_running(socket_path, startup_options, |_| {
        print!(".");
        let _ = std::io::stdout().flush();
    })
    .await
    {
        Ok(EnsureDaemonRunningOutcome::Started) => {
            println!(" done!");
            crate::logging::info("VOICEVOX daemon started successfully");
            Ok(())
        }
        Ok(EnsureDaemonRunningOutcome::AlreadyRunningRecovered) => {
            println!(" done!");
            crate::logging::info("VOICEVOX daemon is already running");
            Ok(())
        }
        Ok(EnsureDaemonRunningOutcome::AlreadyResponsive) => {
            println!(" done!");
            crate::logging::info("VOICEVOX daemon is already running");
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
