use crate::daemon::{DaemonError, DaemonResult};
use anyhow::Result;
use std::fs;
use std::io;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::process;

fn allow_unsafe_path_commands() -> bool {
    std::env::var_os("VOICEVOX_ALLOW_UNSAFE_PATH_COMMANDS").is_some()
}

fn pgrep_command_path() -> &'static str {
    if Path::new("/usr/bin/pgrep").is_file() {
        "/usr/bin/pgrep"
    } else if allow_unsafe_path_commands() {
        "pgrep"
    } else {
        "/usr/bin/pgrep"
    }
}

fn current_uid_string() -> String {
    // SAFETY: `getuid` is thread-safe and has no preconditions.
    unsafe { libc::getuid() }.to_string()
}

fn parse_other_pids(stdout: &[u8]) -> Vec<u32> {
    let current_pid = process::id();

    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .filter(|&pid| pid != current_pid)
        .collect()
}

fn pgrep_voicevox_daemon_output(match_mode: PgrepMatchMode) -> io::Result<process::Output> {
    let mut command = process::Command::new(pgrep_command_path());
    command
        .arg(match_mode.flag())
        .arg("-u")
        .arg(current_uid_string());
    command.arg("voicevox-daemon");
    command.output()
}

#[derive(Clone, Copy)]
enum PgrepMatchMode {
    Exact,
    FullCommandLine,
}

impl PgrepMatchMode {
    const fn flag(self) -> &'static str {
        match self {
            Self::Exact => "-x",
            Self::FullCommandLine => "-f",
        }
    }
}

/// Checks for stale sockets and duplicate daemon processes before startup.
///
/// # Errors
///
/// Returns an error if another daemon instance is already running, a stale socket
/// cannot be removed, or daemon process detection reports a startup conflict.
pub async fn check_and_prevent_duplicate(socket_path: &Path) -> DaemonResult<()> {
    if socket_path.exists() {
        handle_existing_socket(socket_path).await?;
    }
    check_for_other_daemons()?;

    Ok(())
}

async fn handle_existing_socket(socket_path: &Path) -> DaemonResult<()> {
    match tokio::net::UnixStream::connect(socket_path).await {
        Ok(_) => {
            let pid = find_daemon_processes()
                .ok()
                .and_then(|pids| pids.into_iter().next())
                .unwrap_or(0);
            Err(DaemonError::AlreadyRunning { pid })
        }
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(DaemonError::SocketPermissionDenied {
                path: socket_path.to_path_buf(),
            })
        }
        Err(_) => remove_stale_socket(socket_path),
    }
}

fn remove_stale_socket(socket_path: &Path) -> DaemonResult<()> {
    crate::logging::info(&format!(
        "Removing stale socket file: {}",
        socket_path.display()
    ));

    match fs::symlink_metadata(socket_path) {
        Ok(metadata) if !metadata.file_type().is_socket() => {
            return Err(DaemonError::StartupFailed {
                message: format!(
                    "Refusing to remove non-socket path configured as daemon socket: {}",
                    socket_path.display()
                ),
            });
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(DaemonError::StartupFailed {
                message: format!("Failed to inspect socket path: {error}"),
            });
        }
    }

    fs::remove_file(socket_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => DaemonError::SocketPermissionDenied {
            path: socket_path.to_path_buf(),
        },
        _ => DaemonError::StartupFailed {
            message: format!("Failed to remove stale socket: {e}"),
        },
    })
}

fn check_for_other_daemons() -> DaemonResult<()> {
    let output = pgrep_voicevox_daemon_output(PgrepMatchMode::Exact);

    match output {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {
            check_pgrep_output(&output.stdout)
        }
        Ok(_) => Ok(()), // No processes found or empty output
        Err(_) => {
            crate::logging::warn("Could not check for existing processes (pgrep not available)");
            Ok(())
        }
    }
}

fn check_pgrep_output(stdout: &[u8]) -> DaemonResult<()> {
    let other_pids = parse_other_pids(stdout);

    match other_pids.first() {
        Some(&pid) => Err(DaemonError::AlreadyRunning { pid }),
        None => Ok(()),
    }
}

/// Finds running `voicevox-daemon` process IDs for the current user.
///
/// # Errors
///
/// Returns an error only if `pgrep` execution fails unexpectedly in a way surfaced by
/// the process API. Missing processes are reported as an empty list.
pub fn find_daemon_processes() -> Result<Vec<u32>> {
    let output = pgrep_voicevox_daemon_output(PgrepMatchMode::FullCommandLine)?;

    if output.status.success() && !output.stdout.is_empty() {
        Ok(parse_other_pids(&output.stdout))
    } else {
        Ok(Vec::new())
    }
}
