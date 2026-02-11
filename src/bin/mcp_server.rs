use anyhow::Result;
use clap::Parser;
use tokio::process::Command;
use tokio::time::timeout;
use voicevox_cli::client::daemon_client::find_daemon_binary;
use voicevox_cli::daemon::{exit_codes as exit_daemon, startup, DaemonError, DaemonResult};
use voicevox_cli::paths::get_socket_path;

#[derive(Parser, Debug)]
#[command(
    name = "voicevox-mcp-server",
    about = "VOICEVOX MCP Server for AI assistants",
    version
)]
struct Args {}

async fn ensure_daemon_running() -> DaemonResult<()> {
    let socket_path = get_socket_path();

    if try_connect_existing(&socket_path).await? {
        return Ok(());
    }

    if socket_path.exists() {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }

    start_daemon_process(&socket_path).await?;
    wait_for_daemon_ready(&socket_path).await
}

async fn try_connect_existing(socket_path: &std::path::Path) -> DaemonResult<bool> {
    let connect_timeout = startup::connect_timeout();

    match timeout(
        connect_timeout,
        tokio::net::UnixStream::connect(socket_path),
    )
    .await
    {
        Ok(Ok(_)) => Ok(true),
        Ok(Err(_)) | Err(_) => Ok(false),
    }
}

async fn start_daemon_process(socket_path: &std::path::Path) -> DaemonResult<()> {
    let daemon_path = find_daemon_binary()?;

    let output = Command::new(&daemon_path)
        .args(["--start", "--detach"])
        .output()
        .await?;

    match output.status.success() {
        true => Ok(()),
        false => handle_daemon_error(output, socket_path).await,
    }
}

async fn handle_daemon_error(
    output: std::process::Output,
    socket_path: &std::path::Path,
) -> DaemonResult<()> {
    match output.status.code() {
        Some(code) if code == exit_daemon::ALREADY_RUNNING => {
            handle_already_running(socket_path).await
        }
        Some(code) if code == exit_daemon::PERMISSION_DENIED => {
            Err(DaemonError::SocketPermissionDenied {
                path: socket_path.to_path_buf(),
            })
        }
        Some(code) if code == exit_daemon::NO_MODELS => Err(DaemonError::NoModelsAvailable),
        Some(code) if code == exit_daemon::BINARY_NOT_FOUND => {
            Err(DaemonError::DaemonBinaryNotFound)
        }
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(DaemonError::StartupFailed {
                message: stderr.to_string(),
            })
        }
    }
}

async fn handle_already_running(socket_path: &std::path::Path) -> DaemonResult<()> {
    let mut retry_delay = startup::initial_retry_delay();

    for _ in 0..startup::ALREADY_RUNNING_RETRIES {
        tokio::time::sleep(retry_delay).await;
        match tokio::net::UnixStream::connect(socket_path).await.is_ok() {
            true => return Ok(()),
            false => retry_delay *= 2,
        }
    }

    match voicevox_cli::daemon::find_daemon_processes() {
        Ok(pids) => {
            if let Some(&pid) = pids.first() {
                Err(DaemonError::AlreadyRunning { pid })
            } else {
                Err(DaemonError::NotResponding {
                    attempts: startup::ALREADY_RUNNING_RETRIES,
                })
            }
        }
        Err(e) => Err(DaemonError::StartupFailed {
            message: format!("Failed to find daemon processes: {}", e),
        }),
    }
}

async fn wait_for_daemon_ready(socket_path: &std::path::Path) -> DaemonResult<()> {
    let max_attempts = startup::MAX_CONNECT_ATTEMPTS;
    let mut retry_delay = startup::initial_retry_delay();

    for attempt in 0..max_attempts {
        match tokio::net::UnixStream::connect(socket_path).await.is_ok() {
            true => return Ok(()),
            false if attempt < max_attempts - 1 => {
                tokio::time::sleep(retry_delay).await;
                retry_delay = (retry_delay * 2).min(startup::max_retry_delay());
            }
            _ => {}
        }
    }

    Err(DaemonError::NotResponding {
        attempts: max_attempts,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let _args = Args::parse();

    if let Err(e) = ensure_daemon_running().await {
        match e {
            DaemonError::AlreadyRunning { pid } => {
                eprintln!(
                    "Warning: Daemon is running (PID: {}) but may not be responsive.",
                    pid
                );
            }
            DaemonError::SocketPermissionDenied { path } => {
                eprintln!("Warning: Permission denied when starting daemon.");
                eprintln!(
                    "Socket file may be owned by another user: {}",
                    path.display()
                );
                eprintln!("Audio synthesis may not be available.");
            }
            DaemonError::NotResponding { attempts } => {
                eprintln!(
                    "Warning: Daemon started but is not responding after {} attempts.",
                    attempts
                );
                eprintln!("Audio synthesis may not be available.");
            }
            DaemonError::StartupFailed { message } => {
                eprintln!("Warning: Failed to start daemon: {}", message);
                eprintln!("Audio synthesis may not be available.");
            }
            _ => {
                eprintln!("Warning: {}", e);
                eprintln!("Audio synthesis may not be available.");
            }
        }
    }

    voicevox_cli::mcp::run_mcp_server().await?;

    Ok(())
}
