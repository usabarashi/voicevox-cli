use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::net::UnixStream;

use super::policy::{DaemonAutoStartPolicy, DaemonConnectRetryPolicy};
use super::transport::{connect_socket_with_timeout, DAEMON_CONNECTION_TIMEOUT};
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
    connect_after_start_with_retry(socket_path).await
}

async fn connect_after_start_with_retry(socket_path: &Path) -> Result<UnixStream> {
    let auto_start_policy = DaemonAutoStartPolicy::cli_default();
    let retry_policy = DaemonConnectRetryPolicy::default();

    tokio::time::sleep(auto_start_policy.startup_grace_period).await;

    let mut delay = retry_policy.initial_delay;
    let mut last_error = None;

    for attempt in 0..retry_policy.attempts {
        match connect_socket_with_timeout(socket_path, auto_start_policy.final_connection_timeout)
            .await
        {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < retry_policy.attempts {
                    tokio::time::sleep(delay).await;
                    delay = (delay * 2).min(retry_policy.max_delay);
                }
            }
        }
    }

    let attempts = retry_policy.attempts;
    let last_error_text = last_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "unknown error".to_string());

    Err(anyhow!(
        "Daemon started but failed to connect at {} after {} attempts: {}",
        socket_path.display(),
        attempts,
        last_error_text
    ))
}
