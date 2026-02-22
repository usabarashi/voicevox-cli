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

    if try_connect_existing(&socket_path).await {
        return Ok(());
    }

    match tokio::fs::remove_file(&socket_path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(()) | Err(_) => {}
    }

    start_daemon_process(&socket_path).await?;
    wait_for_daemon_ready(&socket_path).await
}

async fn try_connect_existing(socket_path: &std::path::Path) -> bool {
    let connect_timeout = startup::connect_timeout();
    matches!(
        timeout(connect_timeout, tokio::net::UnixStream::connect(socket_path)).await,
        Ok(Ok(_))
    )
}

async fn start_daemon_process(socket_path: &std::path::Path) -> DaemonResult<()> {
    let daemon_path = find_daemon_binary()?;

    let output = Command::new(&daemon_path)
        .args(["--start", "--detach"])
        .output()
        .await?;

    if output.status.success() {
        Ok(())
    } else {
        handle_daemon_error(output, socket_path).await
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
    if wait_for_socket_ready(socket_path, startup::ALREADY_RUNNING_RETRIES, true).await {
        return Ok(());
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
            message: format!("Failed to find daemon processes: {e}"),
        }),
    }
}

async fn wait_for_daemon_ready(socket_path: &std::path::Path) -> DaemonResult<()> {
    let attempts = startup::MAX_CONNECT_ATTEMPTS;
    wait_for_socket_ready(socket_path, attempts, false)
        .await
        .then_some(())
        .ok_or(DaemonError::NotResponding { attempts })
}

async fn wait_for_socket_ready(
    socket_path: &std::path::Path,
    attempts: u32,
    sleep_before_first_check: bool,
) -> bool {
    let mut retry_delay = startup::initial_retry_delay();

    for attempt in 0..attempts {
        if sleep_before_first_check || attempt > 0 {
            tokio::time::sleep(retry_delay).await;
        }

        if tokio::net::UnixStream::connect(socket_path).await.is_ok() {
            return true;
        }

        if attempt + 1 < attempts {
            retry_delay = (retry_delay * 2).min(startup::max_retry_delay());
        }
    }

    false
}

fn print_mcp_warning(message: &str) {
    eprintln!("Warning: {message}");
    eprintln!("Audio synthesis may not be available.");
}

fn print_mcp_warning_with_detail(message: &str, detail: &str) {
    eprintln!("Warning: {message}");
    eprintln!("{detail}");
    eprintln!("Audio synthesis may not be available.");
}

fn warn_nonfatal_daemon_issue(error: &DaemonError) {
    match error {
        DaemonError::AlreadyRunning { pid } => {
            print_mcp_warning(&format!(
                "Daemon is running (PID: {pid}) but may not be responsive."
            ));
        }
        DaemonError::SocketPermissionDenied { path } => {
            print_mcp_warning_with_detail(
                "Permission denied when starting daemon.",
                &format!(
                    "Socket file may be owned by another user: {}",
                    path.display()
                ),
            );
        }
        DaemonError::NotResponding { attempts } => {
            print_mcp_warning(&format!(
                "Daemon started but is not responding after {attempts} attempts."
            ));
        }
        DaemonError::StartupFailed { message } => {
            print_mcp_warning(&format!("Failed to start daemon: {message}"));
        }
        _ => print_mcp_warning(&error.to_string()),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _args = Args::parse();

    if let Err(error) = ensure_daemon_running().await {
        warn_nonfatal_daemon_issue(&error);
    }

    voicevox_cli::mcp::run_mcp_server().await?;

    Ok(())
}
