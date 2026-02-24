use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::net::UnixStream;

use super::transport::{connect_socket_with_timeout, DAEMON_CONNECTION_TIMEOUT};
use super::policy::DaemonAutoStartPolicy;
use crate::daemon::{
    ensure_daemon_running, EnsureDaemonRunningOptions, EnsureDaemonRunningOutcome,
};

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

    let policy = DaemonAutoStartPolicy::cli_default();
    let startup_options: EnsureDaemonRunningOptions = policy.ensure_running;

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
    let policy = DaemonAutoStartPolicy::cli_default();
    tokio::time::sleep(policy.startup_grace_period).await;

    connect_socket_with_timeout(socket_path, policy.final_connection_timeout)
        .await
        .map_err(|e| {
            anyhow!(
                "Daemon started but failed to connect at {}: {e}",
                socket_path.display()
            )
        })
}
