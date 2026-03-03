use std::path::Path;
use std::path::PathBuf;

use tokio::process::Command;

use super::{exit_codes as exit_daemon, DaemonError, DaemonResult};

pub enum StartDaemonOutcome {
    Started,
    AlreadyRunning,
}

/// Finds the daemon executable path using the current binary location and common fallbacks.
///
/// # Errors
///
/// Returns `DaemonBinaryNotFound` if no usable `voicevox-daemon` binary can be found.
pub fn find_daemon_binary() -> Result<PathBuf, DaemonError> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut daemon_path = current_exe;
        daemon_path.set_file_name("voicevox-daemon");
        if daemon_path.is_file() {
            return Ok(daemon_path);
        }
    }

    if std::env::var_os(crate::config::ENV_VOICEVOX_ALLOW_UNSAFE_DAEMON_LOOKUP).is_some() {
        return [
            PathBuf::from("./target/debug/voicevox-daemon"),
            PathBuf::from("./target/release/voicevox-daemon"),
        ]
        .into_iter()
        .find(|p| p.exists())
        .or_else(|| find_in_path("voicevox-daemon"))
        .ok_or(DaemonError::DaemonBinaryNotFound);
    }

    Err(DaemonError::DaemonBinaryNotFound)
}

fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    std::env::var_os(crate::config::ENV_PATH).and_then(|path_var| {
        std::env::split_paths(&path_var)
            .map(|dir| dir.join(binary_name))
            .find(|candidate| candidate.is_file())
    })
}

pub async fn start_daemon_detached(socket_path: Option<&Path>) -> DaemonResult<StartDaemonOutcome> {
    let daemon_path = find_daemon_binary()?;

    let mut command = Command::new(&daemon_path);
    command.args(["--start", "--detach"]);
    if let Some(socket_path) = socket_path {
        command.arg("--socket-path").arg(socket_path);
    }

    let output = command.output().await?;
    classify_start_output(output, socket_path)
}

fn classify_start_output(
    output: std::process::Output,
    socket_path: Option<&Path>,
) -> DaemonResult<StartDaemonOutcome> {
    if output.status.success() {
        return Ok(StartDaemonOutcome::Started);
    }

    match output.status.code() {
        Some(code) if code == exit_daemon::ALREADY_RUNNING => {
            Ok(StartDaemonOutcome::AlreadyRunning)
        }
        Some(code) if code == exit_daemon::PERMISSION_DENIED => {
            Err(DaemonError::SocketPermissionDenied {
                path: socket_path
                    .map(Path::to_path_buf)
                    .unwrap_or_else(crate::infrastructure::paths::get_socket_path),
            })
        }
        Some(code) if code == exit_daemon::NO_MODELS => Err(DaemonError::NoModelsAvailable),
        Some(code) if code == exit_daemon::BINARY_NOT_FOUND => {
            Err(DaemonError::DaemonBinaryNotFound)
        }
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(DaemonError::StartupFailed {
                message: stderr.trim().to_string(),
            })
        }
    }
}
