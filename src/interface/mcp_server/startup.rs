use crate::domain::startup_phase::McpStartupPhase;
use crate::infrastructure::daemon::{ensure_daemon_running, DaemonError, DaemonResult};
use crate::infrastructure::paths::get_socket_path;
use crate::interface::cli::DaemonAutoStartPolicy;
use crate::interface::{AppOutput, StdAppOutput};
use anyhow::Result;
use std::io;
use std::path::Path;
use std::time::Duration;

async fn attempt_mcp_daemon_start(socket_path: &Path) -> DaemonResult<()> {
    let policy = DaemonAutoStartPolicy::mcp_default();
    ensure_daemon_running(socket_path, policy.ensure_running, |_| {})
        .await
        .map(|_| ())
}

async fn wait_for_process_exit(pid: u32, attempts: u32, delay: Duration) -> bool {
    for _ in 0..attempts {
        let status = {
            // SAFETY: `kill` with signal 0 only probes process existence.
            unsafe { libc::kill(pid as i32, 0) }
        };
        if status != 0 {
            return true;
        }
        tokio::time::sleep(delay).await;
    }
    false
}

async fn terminate_stuck_daemon(pid: u32) -> io::Result<()> {
    let term_status = {
        // SAFETY: Best-effort signal delivery to an existing pid.
        unsafe { libc::kill(pid as i32, libc::SIGTERM) }
    };
    if term_status != 0 {
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::NotFound {
            return Err(err);
        }
        return Ok(());
    }

    if wait_for_process_exit(pid, 10, Duration::from_millis(100)).await {
        return Ok(());
    }

    let kill_status = {
        // SAFETY: Fallback for unresponsive daemon process.
        unsafe { libc::kill(pid as i32, libc::SIGKILL) }
    };
    if kill_status != 0 {
        let err = io::Error::last_os_error();
        if err.kind() != io::ErrorKind::NotFound {
            return Err(err);
        }
    }
    Ok(())
}

async fn recover_stuck_daemon_and_retry(pid: u32, socket_path: &Path) -> DaemonResult<()> {
    terminate_stuck_daemon(pid)
        .await
        .map_err(|error| DaemonError::StartupFailed {
            message: format!("Failed to terminate unresponsive daemon (PID: {pid}): {error}"),
        })?;

    attempt_mcp_daemon_start(socket_path).await
}

async fn run_mcp_startup_phase(
    phase: McpStartupPhase,
    socket_path: &Path,
    output: &dyn AppOutput,
) -> DaemonResult<Option<McpStartupPhase>> {
    match phase {
        McpStartupPhase::InitialStart => match attempt_mcp_daemon_start(socket_path).await {
            Ok(()) => Ok(None),
            Err(DaemonError::AlreadyRunning { pid }) => {
                Ok(Some(McpStartupPhase::RecoverAlreadyRunning { pid }))
            }
            Err(error) => Err(error),
        },
        McpStartupPhase::RecoverAlreadyRunning { pid } => {
            output.info(&format!(
                "Recovering possibly stuck daemon process (PID: {pid})..."
            ));
            recover_stuck_daemon_and_retry(pid, socket_path).await?;
            Ok(None)
        }
    }
}

async fn ensure_daemon_running_for_mcp(output: &dyn AppOutput) -> DaemonResult<()> {
    let socket_path = get_socket_path();
    let mut phase = McpStartupPhase::InitialStart;

    loop {
        match run_mcp_startup_phase(phase, &socket_path, output).await? {
            None => return Ok(()),
            Some(next) => {
                phase = next;
            }
        }
    }
}

fn print_mcp_warning(message: &str, output: &dyn AppOutput) {
    output.error(&format!("Warning: {message}"));
    output.error("Audio synthesis may not be available.");
}

fn print_mcp_warning_with_detail(message: &str, detail: &str, output: &dyn AppOutput) {
    output.error(&format!("Warning: {message}"));
    output.error(detail);
    output.error("Audio synthesis may not be available.");
}

fn warn_nonfatal_daemon_issue(error: &DaemonError, output: &dyn AppOutput) {
    match error {
        DaemonError::AlreadyRunning { pid } => {
            print_mcp_warning(
                &format!("Daemon is running (PID: {pid}) but may not be responsive."),
                output,
            );
        }
        DaemonError::SocketPermissionDenied { path } => {
            print_mcp_warning_with_detail(
                "Permission denied when starting daemon.",
                &format!(
                    "Socket file may be owned by another user: {}",
                    path.display()
                ),
                output,
            );
        }
        DaemonError::NotResponding { attempts } => {
            print_mcp_warning(
                &format!("Daemon started but is not responding after {attempts} attempts."),
                output,
            );
        }
        DaemonError::StartupFailed { message } => {
            print_mcp_warning(&format!("Failed to start daemon: {message}"), output);
        }
        _ => print_mcp_warning(&error.to_string(), output),
    }
}

/// Runs the MCP server application flow, attempting daemon startup first.
///
/// # Errors
///
/// Returns an error only if the MCP stdio server itself fails.
pub async fn run_mcp_server_app() -> Result<()> {
    let output = StdAppOutput;
    run_mcp_server_app_with_output(&output).await
}

pub async fn run_mcp_server_app_with_output(output: &dyn AppOutput) -> Result<()> {
    if let Err(error) = ensure_daemon_running_for_mcp(output).await {
        warn_nonfatal_daemon_issue(&error, output);
    }

    crate::interface::mcp_server::run_mcp_server().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::output::BufferAppOutput;
    use std::path::PathBuf;

    #[test]
    fn warns_for_not_responding_daemon() {
        let output = BufferAppOutput::default();

        warn_nonfatal_daemon_issue(&DaemonError::NotResponding { attempts: 5 }, &output);

        let errors = output.errors().join("\n");
        assert!(errors.contains("Warning: Daemon started but is not responding after 5 attempts."));
        assert!(errors.contains("Audio synthesis may not be available."));
    }

    #[test]
    fn warns_with_detail_for_socket_permission_issue() {
        let output = BufferAppOutput::default();
        let socket_path = PathBuf::from("/tmp/voicevox.sock");

        warn_nonfatal_daemon_issue(
            &DaemonError::SocketPermissionDenied { path: socket_path },
            &output,
        );

        let errors = output.errors().join("\n");
        assert!(errors.contains("Warning: Permission denied when starting daemon."));
        assert!(errors.contains("Socket file may be owned by another user: /tmp/voicevox.sock"));
        assert!(errors.contains("Audio synthesis may not be available."));
    }
}
