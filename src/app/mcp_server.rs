use crate::daemon::{startup, DaemonError, DaemonResult, StartDaemonOutcome};
use crate::paths::get_socket_path;
use anyhow::Result;

async fn remove_stale_socket_if_present(socket_path: &std::path::Path) {
    match tokio::fs::remove_file(socket_path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(()) => {}
        Err(error) => {
            eprintln!(
                "Warning: failed to remove stale socket candidate {}: {error}",
                socket_path.display()
            );
        }
    }
}

async fn try_connect_existing(socket_path: &std::path::Path) -> bool {
    crate::daemon::socket_probe::try_connect_with_timeout(socket_path, startup::connect_timeout())
        .await
}

async fn handle_already_running(socket_path: &std::path::Path) -> DaemonResult<()> {
    if crate::daemon::socket_probe::wait_for_socket_ready_with_backoff(
        socket_path,
        startup::ALREADY_RUNNING_RETRIES,
        startup::initial_retry_delay(),
        startup::max_retry_delay(),
        true,
        |_| {},
    )
    .await
    {
        return Ok(());
    }

    match crate::daemon::find_daemon_processes() {
        Ok(pids) => {
            if let Some(&pid) = pids.first() {
                Err(DaemonError::AlreadyRunning { pid })
            } else {
                Err(DaemonError::NotResponding {
                    attempts: startup::ALREADY_RUNNING_RETRIES,
                })
            }
        }
        Err(error) => Err(DaemonError::StartupFailed {
            message: format!("Failed to find daemon processes: {error}"),
        }),
    }
}

async fn start_daemon_process(socket_path: &std::path::Path) -> DaemonResult<()> {
    match crate::daemon::start_daemon_detached(None).await? {
        StartDaemonOutcome::Started => Ok(()),
        StartDaemonOutcome::AlreadyRunning => handle_already_running(socket_path).await,
    }
}

async fn wait_for_daemon_ready(socket_path: &std::path::Path) -> DaemonResult<()> {
    let attempts = startup::MAX_CONNECT_ATTEMPTS;
    crate::daemon::socket_probe::wait_for_socket_ready_with_backoff(
        socket_path,
        attempts,
        startup::initial_retry_delay(),
        startup::max_retry_delay(),
        false,
        |_| {},
    )
    .await
    .then_some(())
    .ok_or(DaemonError::NotResponding { attempts })
}

async fn ensure_daemon_running_for_mcp() -> DaemonResult<()> {
    let socket_path = get_socket_path();

    if try_connect_existing(&socket_path).await {
        return Ok(());
    }

    remove_stale_socket_if_present(&socket_path).await;
    start_daemon_process(&socket_path).await?;
    wait_for_daemon_ready(&socket_path).await
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

/// Runs the MCP server application flow, attempting daemon startup first.
///
/// # Errors
///
/// Returns an error only if the MCP stdio server itself fails.
pub async fn run_mcp_server_app() -> Result<()> {
    if let Err(error) = ensure_daemon_running_for_mcp().await {
        warn_nonfatal_daemon_issue(&error);
    }

    crate::mcp::run_mcp_server().await
}
