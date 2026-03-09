use crate::infrastructure::daemon::client::DaemonAutoStartPolicy;
use crate::infrastructure::daemon::{
    DaemonError, DaemonResult, ensure_daemon_running,
    recover_stuck_daemon_and_retry as recover_stuck_daemon_and_retry_impl,
};
use crate::infrastructure::paths::get_socket_path;
use crate::interface::{AppOutput, StdAppOutput};
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpStartupPhase {
    InitialStart,
    RecoverAlreadyRunning { pid: u32 },
}

async fn attempt_mcp_daemon_start(socket_path: &Path) -> DaemonResult<()> {
    let policy = DaemonAutoStartPolicy::mcp_default();
    ensure_daemon_running(socket_path, policy.ensure_running, |_| {})
        .await
        .map(|_| ())
}

async fn recover_stuck_daemon_for_mcp(pid: u32, socket_path: &Path) -> DaemonResult<()> {
    let policy = DaemonAutoStartPolicy::mcp_default();
    recover_stuck_daemon_and_retry_impl(pid, socket_path, policy.ensure_running)
        .await
        .map(|_| ())
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
            recover_stuck_daemon_for_mcp(pid, socket_path).await?;
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
