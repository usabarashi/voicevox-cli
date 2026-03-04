use anyhow::{Result, anyhow};
use std::path::Path;
use tokio::net::UnixStream;

use super::policy::{DaemonAutoStartPolicy, DaemonConnectRetryPolicy};
use super::transport::{
    DAEMON_CONNECTION_TIMEOUT, connect_socket_with_timeout, connect_with_retry,
};
use crate::infrastructure::daemon::{
    EnsureDaemonRunningOptions, EnsureDaemonRunningOutcome, ensure_daemon_running,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupPhase {
    InitialConnect,
    ValidateModels,
    StartDaemon,
    ConnectRetry,
}

async fn connect_once(socket_path: &Path) -> Result<UnixStream> {
    connect_socket_with_timeout(socket_path, DAEMON_CONNECTION_TIMEOUT).await
}

fn validate_startup_preconditions() -> Result<()> {
    let missing = crate::infrastructure::download::missing_startup_resources();
    if !missing.is_empty() {
        let resources = missing.join(", ");
        return Err(anyhow!(
            "Required VOICEVOX resources are missing ({resources}). Please run 'voicevox-setup'."
        ));
    }
    Ok(())
}

async fn start_daemon_automatically(socket_path: &Path) -> Result<()> {
    crate::infrastructure::logging::info(
        "Starting VOICEVOX daemon (first startup may take a few seconds)...",
    );
    crate::infrastructure::logging::info("  Resources:");
    crate::infrastructure::logging::info(&format!(
        "    ONNX Runtime: {}",
        crate::infrastructure::paths::find_onnxruntime()?.display()
    ));
    crate::infrastructure::logging::info(&format!(
        "    OpenJTalk Dictionary: {}",
        crate::infrastructure::paths::find_openjtalk_dict()?.display()
    ));

    let models_dir = crate::infrastructure::paths::find_models_dir()?;
    let models = crate::infrastructure::voicevox::scan_available_models()?;
    crate::infrastructure::logging::info(&format!(
        "    Voice Models: {} in {}",
        models.len(),
        models_dir.display()
    ));
    crate::infrastructure::logging::info(
        "  Building voice model mappings (this may take a moment)...",
    );
    crate::infrastructure::logging::info("  Starting daemon process...");

    let policy = DaemonAutoStartPolicy::cli_default();
    let startup_options: EnsureDaemonRunningOptions = policy.ensure_running;

    match ensure_daemon_running(socket_path, startup_options, |_| {}).await {
        Ok(EnsureDaemonRunningOutcome::Started) => {
            crate::infrastructure::logging::info("VOICEVOX daemon started successfully");
            Ok(())
        }
        Ok(EnsureDaemonRunningOutcome::AlreadyRunningRecovered) => {
            crate::infrastructure::logging::info("VOICEVOX daemon is already running");
            Ok(())
        }
        Ok(EnsureDaemonRunningOutcome::AlreadyResponsive) => {
            crate::infrastructure::logging::info("VOICEVOX daemon is already running");
            Ok(())
        }
        Err(error) => Err(anyhow!("Failed to execute daemon: {error}")),
    }
}

pub(crate) async fn connect_or_start(socket_path: &Path) -> Result<UnixStream> {
    let mut phase = StartupPhase::InitialConnect;

    loop {
        let (connected, next_phase) = run_startup_phase(phase, socket_path).await?;
        if let Some(stream) = connected {
            return Ok(stream);
        }
        if let Some(next) = next_phase {
            phase = next;
        }
    }
}

async fn run_startup_phase(
    phase: StartupPhase,
    socket_path: &Path,
) -> Result<(Option<UnixStream>, Option<StartupPhase>)> {
    match phase {
        StartupPhase::InitialConnect => match connect_once(socket_path).await {
            Ok(stream) => Ok((Some(stream), None)),
            Err(_) => Ok((None, Some(StartupPhase::ValidateModels))),
        },
        StartupPhase::ValidateModels => {
            validate_startup_preconditions()?;
            Ok((None, Some(StartupPhase::StartDaemon)))
        }
        StartupPhase::StartDaemon => {
            start_daemon_automatically(socket_path).await?;
            Ok((None, Some(StartupPhase::ConnectRetry)))
        }
        StartupPhase::ConnectRetry => {
            let stream = connect_after_start_with_retry(socket_path).await?;
            Ok((Some(stream), None))
        }
    }
}

async fn connect_after_start_with_retry(socket_path: &Path) -> Result<UnixStream> {
    let auto_start_policy = DaemonAutoStartPolicy::cli_default();
    let retry_policy = DaemonConnectRetryPolicy::default();

    tokio::time::sleep(auto_start_policy.startup_grace_period).await;

    connect_with_retry(
        socket_path,
        auto_start_policy.final_connection_timeout,
        retry_policy,
    )
    .await
    .map_err(|error| {
        let attempts = retry_policy.attempts;
        anyhow!(
            "Daemon started but failed to connect at {} after {} attempts: {}",
            socket_path.display(),
            attempts,
            error
        )
    })
}
