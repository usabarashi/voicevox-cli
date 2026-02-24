use anyhow::Result;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use tokio::net::UnixStream;

use crate::app::{AppOutput, StdAppOutput};
use crate::daemon::{check_and_prevent_duplicate, exit_codes as exit_daemon, DaemonError};

#[derive(Clone, Copy)]
pub enum StartMode {
    Foreground,
    Detached,
}

impl StartMode {
    #[must_use]
    pub fn from_flags(foreground: bool, detach: bool) -> Self {
        if detach && !foreground {
            Self::Detached
        } else {
            Self::Foreground
        }
    }

    #[must_use]
    pub const fn is_foreground(self) -> bool {
        matches!(self, Self::Foreground)
    }

    const fn should_detach(self) -> bool {
        matches!(self, Self::Detached)
    }
}

#[derive(Clone, Copy)]
pub enum ControlCommand {
    None,
    Stop,
    Status,
    Restart,
}

#[derive(Clone, Copy)]
pub struct DaemonRunFlags {
    pub start_mode: StartMode,
    pub mode_flag_explicit: bool,
    pub start: bool,
    pub control: ControlCommand,
}

#[derive(Clone, Copy)]
enum Invocation {
    ShowUsage,
    Control(ControlAction),
    Start,
}

#[derive(Clone, Copy)]
enum ControlAction {
    Stop,
    Status,
    Restart,
}

impl Invocation {
    const fn from_flags(flags: DaemonRunFlags) -> Self {
        match flags.control {
            ControlCommand::Stop => Self::Control(ControlAction::Stop),
            ControlCommand::Status => Self::Control(ControlAction::Status),
            ControlCommand::Restart => Self::Control(ControlAction::Restart),
            ControlCommand::None if !flags.start && !flags.mode_flag_explicit => Self::ShowUsage,
            ControlCommand::None => Self::Start,
        }
    }
}

enum ExecutionDecision {
    Continue,
    Exit(i32),
}

impl ExecutionDecision {
    const fn exit(code: i32) -> Self {
        Self::Exit(code)
    }
}

fn print_usage_banner(output: &dyn AppOutput) {
    output.info(&format!("VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION")));
    output.info("\nDaemon Operations:");
    output.info("  --start     Start the daemon (default)");
    output.info("  --stop      Stop the running daemon");
    output.info("  --status    Check daemon status");
    output.info("  --restart   Restart the daemon");
    output.info("\nExecution Modes:");
    output.info("  --foreground Run in foreground (for development)");
    output.info("  --detach     Run as background process");
    output.info("\nUse --help for all options");
}

async fn maybe_handle_control_commands(
    socket_path: &Path,
    flags: DaemonRunFlags,
    output: &dyn AppOutput,
) -> Result<bool> {
    match Invocation::from_flags(flags) {
        Invocation::Control(action) => {
            run_control_action(action, socket_path, output).await?;
            Ok(!matches!(action, ControlAction::Restart))
        }
        Invocation::ShowUsage => {
            print_usage_banner(output);
            Ok(true)
        }
        Invocation::Start => Ok(false),
    }
}

async fn run_control_action(
    action: ControlAction,
    socket_path: &Path,
    output: &dyn AppOutput,
) -> Result<()> {
    match action {
        ControlAction::Stop => handle_stop_daemon(socket_path, output).await,
        ControlAction::Status => handle_status_daemon(socket_path, output).await,
        ControlAction::Restart => {
            output.info("Restarting daemon...");
            let _ = handle_stop_daemon(socket_path, output).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            Ok(())
        }
    }
}

async fn maybe_detach(
    socket_path: &Path,
    flags: DaemonRunFlags,
    output: &dyn AppOutput,
) -> ExecutionDecision {
    if !flags.start_mode.should_detach() {
        return ExecutionDecision::Continue;
    }

    output.info("Starting daemon in detached mode...");

    let mut args = std::env::args()
        .filter(|arg| arg != "--detach" && arg != "-d")
        .collect::<Vec<_>>();
    args.push(String::from("--foreground"));

    let child = ProcessCommand::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();

    match child {
        Ok(mut child) => {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            match child.try_wait() {
                Ok(None) => {
                    output.info("VOICEVOX daemon started successfully in background");
                    output.info(&format!("   Socket: {}", socket_path.display()));
                    ExecutionDecision::exit(0)
                }
                Ok(Some(status)) => {
                    output.error(&format!("Daemon failed to start: exit code {status}"));
                    ExecutionDecision::exit(1)
                }
                Err(error) => {
                    output.error(&format!("Failed to check daemon status: {error}"));
                    ExecutionDecision::exit(1)
                }
            }
        }
        Err(error) => {
            output.error(&format!("Failed to spawn daemon process: {error}"));
            ExecutionDecision::exit(1)
        }
    }
}

async fn ensure_startup_preconditions(socket_path: &Path) -> Result<(), DaemonError> {
    check_and_prevent_duplicate(socket_path).await
}

const fn startup_error_exit_code(error: &DaemonError) -> i32 {
    match error {
        DaemonError::AlreadyRunning { .. } => exit_daemon::ALREADY_RUNNING,
        DaemonError::SocketPermissionDenied { .. } => exit_daemon::PERMISSION_DENIED,
        DaemonError::NoModelsAvailable => exit_daemon::NO_MODELS,
        _ => exit_daemon::FAILURE,
    }
}

fn report_startup_error(error: &DaemonError, output: &dyn AppOutput) -> i32 {
    match error {
        DaemonError::AlreadyRunning { pid } => {
            output.error(&format!("VOICEVOX daemon is already running (PID: {pid})"));
            output.error("   Use 'voicevox-daemon --stop' to stop it.");
        }
        DaemonError::SocketPermissionDenied { path } => {
            output.error("Permission denied: Socket file is owned by another user");
            output.error(&format!("   Socket path: {}", path.display()));
            output.error("   Please remove the file manually and try again.");
        }
        _ => output.error(&error.to_string()),
    }
    startup_error_exit_code(error)
}

fn print_daemon_start_banner(socket_path: &Path, output: &dyn AppOutput) {
    output.info(&format!("VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION")));
    output.info("Starting user daemon...");
    output.info(&format!(
        "Socket: {} (user-specific)",
        socket_path.display()
    ));
    output.info("Models: Load and unload per request (no caching)");
}

async fn daemon_is_responsive(socket_path: &Path) -> bool {
    UnixStream::connect(socket_path).await.is_ok()
}

fn print_socket_path_line(socket_path: &Path, output: &dyn AppOutput) {
    output.info(&format!("Socket: {}", socket_path.display()));
}

fn print_socket_not_running(socket_path: &Path, output: &dyn AppOutput) {
    output.info("Daemon is not running");
    output.info(&format!("   Socket: {}", socket_path.display()));
}

fn print_pid_memory_info(pid_num: u32, output: &dyn AppOutput) {
    let ps_output = std::process::Command::new("ps")
        .args(["-p", &pid_num.to_string(), "-o", "rss,pmem,time"])
        .output();

    let Ok(ps_output) = ps_output else {
        return;
    };
    if !ps_output.status.success() {
        return;
    }

    let info = String::from_utf8_lossy(&ps_output.stdout);
    if let Some(line) = info.lines().nth(1).map(str::trim) {
        output.info(&format!("Memory Info: {line}"));
    }
}

async fn handle_stop_daemon(socket_path: &Path, output: &dyn AppOutput) -> Result<()> {
    output.info("Stopping VOICEVOX daemon...");

    if !daemon_is_responsive(socket_path).await {
        print_socket_not_running(socket_path, output);
        return Ok(());
    }

    let pids = match crate::daemon::process::find_daemon_processes() {
        Ok(pids) => pids,
        Err(error) => {
            output.info(&format!("Failed to find daemon process: {error}"));
            output.info("   Try manual: pkill -f -u $(id -u) voicevox-daemon");
            return Ok(());
        }
    };

    if pids.is_empty() {
        output.info("No daemon process found");
        return Ok(());
    }

    for pid_num in pids {
        stop_daemon_process(pid_num, socket_path, output).await;
    }

    Ok(())
}

async fn stop_daemon_process(pid: u32, socket_path: &Path, output: &dyn AppOutput) {
    let kill_result = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();

    match kill_result {
        Ok(status) if status.success() => {
            output.info(&format!("Daemon stopped (PID: {pid})"));
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            if daemon_is_responsive(socket_path).await {
                output.info("Daemon may still be running");
            } else {
                output.info("Socket cleanup confirmed");
            }
        }
        _ => {
            output.info(&format!("Failed to stop daemon (PID: {pid})"));
            output.info(&format!("   Try: kill -9 {pid}"));
        }
    }
}

async fn handle_status_daemon(socket_path: &Path, output: &dyn AppOutput) -> Result<()> {
    output.info("VOICEVOX Daemon Status");
    output.info("========================");

    if daemon_is_responsive(socket_path).await {
        output.info("Status:  Running and responsive");
        print_socket_path_line(socket_path, output);

        if let Ok(pids) = crate::daemon::process::find_daemon_processes() {
            for pid_num in pids {
                output.info(&format!("Process ID: {pid_num}"));
                print_pid_memory_info(pid_num, output);
            }
        }
    } else {
        output.info("Status:  Not running");
        print_socket_path_line(socket_path, output);
    }

    Ok(())
}

/// Executes daemon CLI flow from already-parsed flags and exits the process when required.
///
/// # Errors
///
/// Returns an error if command dispatch or daemon runtime fails.
pub async fn run_daemon_cli(socket_path: PathBuf, flags: DaemonRunFlags) -> Result<()> {
    let output = StdAppOutput;
    run_daemon_cli_with_output(socket_path, flags, &output).await
}

pub async fn run_daemon_cli_with_output(
    socket_path: PathBuf,
    flags: DaemonRunFlags,
    output: &dyn AppOutput,
) -> Result<()> {
    if maybe_handle_control_commands(&socket_path, flags, output).await? {
        return Ok(());
    }

    if let ExecutionDecision::Exit(code) = maybe_detach(&socket_path, flags, output).await {
        std::process::exit(code);
    }

    if let Err(error) = ensure_startup_preconditions(&socket_path).await {
        std::process::exit(report_startup_error(&error, output));
    }

    print_daemon_start_banner(&socket_path, output);
    crate::daemon::run_daemon(socket_path, flags.start_mode.is_foreground()).await
}
