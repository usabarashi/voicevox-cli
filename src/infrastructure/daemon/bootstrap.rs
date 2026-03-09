use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::time::Duration;

use super::{
    DaemonError, DaemonResult, StartDaemonOutcome, socket_probe, start_daemon_detached, startup,
};

#[derive(Debug, Clone, Copy)]
pub struct EnsureDaemonRunningOptions {
    pub remove_stale_socket: bool,
    pub connect_timeout: std::time::Duration,
    pub wait_attempts: u32,
    pub initial_retry_delay: std::time::Duration,
    pub max_retry_delay: std::time::Duration,
    pub sleep_before_first_check: bool,
}

impl Default for EnsureDaemonRunningOptions {
    fn default() -> Self {
        Self {
            remove_stale_socket: false,
            connect_timeout: startup::connect_timeout(),
            wait_attempts: startup::MAX_CONNECT_ATTEMPTS,
            initial_retry_delay: startup::initial_retry_delay(),
            max_retry_delay: startup::max_retry_delay(),
            sleep_before_first_check: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnsureDaemonRunningOutcome {
    AlreadyResponsive,
    Started,
    AlreadyRunningRecovered,
}

async fn remove_stale_socket_if_requested(
    socket_path: &Path,
    remove_stale_socket: bool,
    connect_timeout: std::time::Duration,
) -> DaemonResult<()> {
    if !remove_stale_socket {
        return Ok(());
    }

    // Mirror DaemonStartup.tla: never remove a responsive socket.
    if socket_probe::try_connect_with_timeout(socket_path, connect_timeout).await {
        return Ok(());
    }

    match tokio::fs::symlink_metadata(socket_path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(metadata) if metadata.file_type().is_socket() => {
            // TOCTOU guard: re-check responsiveness immediately before cleanup.
            if socket_probe::try_connect_with_timeout(socket_path, connect_timeout).await {
                return Ok(());
            }

            match crate::infrastructure::daemon::find_daemon_processes() {
                Ok(pids) => {
                    if let Some(&pid) = pids.first() {
                        return Err(DaemonError::AlreadyRunning { pid });
                    }
                }
                Err(error) => {
                    return Err(DaemonError::StartupFailed {
                        message: format!(
                            "Failed to inspect daemon processes before stale cleanup: {error}"
                        ),
                    });
                }
            }

            tokio::fs::remove_file(socket_path).await.map_err(|error| {
                DaemonError::StartupFailed {
                    message: format!(
                        "Failed to remove stale socket {}: {error}",
                        socket_path.display()
                    ),
                }
            })?;
            Ok(())
        }
        Ok(_) => Err(DaemonError::StartupFailed {
            message: format!(
                "Refusing to remove non-socket path configured as daemon socket: {}",
                socket_path.display()
            ),
        }),
        Err(error) => Err(DaemonError::StartupFailed {
            message: format!(
                "Failed to inspect socket path {}: {error}",
                socket_path.display()
            ),
        }),
    }
}

async fn wait_ready_with_options<F>(
    socket_path: &Path,
    options: EnsureDaemonRunningOptions,
    on_retry: F,
) -> bool
where
    F: FnMut(u32),
{
    socket_probe::wait_for_socket_ready_with_backoff(
        socket_path,
        options.wait_attempts,
        options.initial_retry_delay,
        options.max_retry_delay,
        options.sleep_before_first_check,
        on_retry,
    )
    .await
}

async fn handle_already_running<F>(
    socket_path: &Path,
    options: EnsureDaemonRunningOptions,
    on_retry: F,
) -> DaemonResult<EnsureDaemonRunningOutcome>
where
    F: FnMut(u32),
{
    if wait_ready_with_options(socket_path, options, on_retry).await {
        Ok(EnsureDaemonRunningOutcome::AlreadyRunningRecovered)
    } else {
        let attempts = options.wait_attempts;
        match crate::infrastructure::daemon::find_daemon_processes() {
            Ok(pids) => {
                if let Some(&pid) = pids.first() {
                    Err(DaemonError::AlreadyRunning { pid })
                } else {
                    Err(DaemonError::NotResponding { attempts })
                }
            }
            Err(error) => Err(DaemonError::StartupFailed {
                message: format!("Failed to find daemon processes: {error}"),
            }),
        }
    }
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

async fn terminate_stuck_daemon(pid: u32) -> std::io::Result<()> {
    let term_status = {
        // SAFETY: Best-effort signal delivery to an existing pid.
        unsafe { libc::kill(pid as i32, libc::SIGTERM) }
    };
    if term_status != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::NotFound {
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
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(err);
        }
    }
    Ok(())
}

pub async fn recover_stuck_daemon_and_retry(
    pid: u32,
    socket_path: &Path,
    options: EnsureDaemonRunningOptions,
) -> DaemonResult<EnsureDaemonRunningOutcome> {
    terminate_stuck_daemon(pid)
        .await
        .map_err(|error| DaemonError::StartupFailed {
            message: format!("Failed to terminate unresponsive daemon (PID: {pid}): {error}"),
        })?;
    ensure_daemon_running(socket_path, options, |_| {}).await
}

pub async fn ensure_daemon_running<F>(
    socket_path: &Path,
    options: EnsureDaemonRunningOptions,
    on_retry: F,
) -> DaemonResult<EnsureDaemonRunningOutcome>
where
    F: FnMut(u32),
{
    if socket_probe::try_connect_with_timeout(socket_path, options.connect_timeout).await {
        return Ok(EnsureDaemonRunningOutcome::AlreadyResponsive);
    }

    remove_stale_socket_if_requested(
        socket_path,
        options.remove_stale_socket,
        options.connect_timeout,
    )
    .await?;

    match start_daemon_detached(Some(socket_path)).await? {
        StartDaemonOutcome::Started => wait_ready_with_options(socket_path, options, on_retry)
            .await
            .then_some(EnsureDaemonRunningOutcome::Started)
            .ok_or(DaemonError::NotResponding {
                attempts: options.wait_attempts,
            }),
        StartDaemonOutcome::AlreadyRunning => {
            handle_already_running(socket_path, options, on_retry).await
        }
    }
}
