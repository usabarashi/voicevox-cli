use anyhow::Result;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use tokio::net::UnixStream;

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

fn print_usage_banner() {
    println!("VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION"));
    println!("\nDaemon Operations:");
    println!("  --start     Start the daemon (default)");
    println!("  --stop      Stop the running daemon");
    println!("  --status    Check daemon status");
    println!("  --restart   Restart the daemon");
    println!("\nExecution Modes:");
    println!("  --foreground Run in foreground (for development)");
    println!("  --detach     Run as background process");
    println!("\nUse --help for all options");
}

async fn maybe_handle_control_commands(socket_path: &Path, flags: DaemonRunFlags) -> Result<bool> {
    match Invocation::from_flags(flags) {
        Invocation::Control(action) => {
            run_control_action(action, socket_path).await?;
            Ok(!matches!(action, ControlAction::Restart))
        }
        Invocation::ShowUsage => {
            print_usage_banner();
            Ok(true)
        }
        Invocation::Start => Ok(false),
    }
}

async fn run_control_action(action: ControlAction, socket_path: &Path) -> Result<()> {
    match action {
        ControlAction::Stop => handle_stop_daemon(socket_path).await,
        ControlAction::Status => handle_status_daemon(socket_path).await,
        ControlAction::Restart => {
            println!("Restarting daemon...");
            let _ = handle_stop_daemon(socket_path).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            Ok(())
        }
    }
}

async fn maybe_detach(socket_path: &Path, flags: DaemonRunFlags) -> ExecutionDecision {
    if !flags.start_mode.should_detach() {
        return ExecutionDecision::Continue;
    }

    println!("Starting daemon in detached mode...");

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
                    println!("VOICEVOX daemon started successfully in background");
                    println!("   Socket: {}", socket_path.display());
                    ExecutionDecision::exit(0)
                }
                Ok(Some(status)) => {
                    eprintln!("Daemon failed to start: exit code {status}");
                    ExecutionDecision::exit(1)
                }
                Err(error) => {
                    eprintln!("Failed to check daemon status: {error}");
                    ExecutionDecision::exit(1)
                }
            }
        }
        Err(error) => {
            eprintln!("Failed to spawn daemon process: {error}");
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

fn report_startup_error(error: &DaemonError) -> i32 {
    match error {
        DaemonError::AlreadyRunning { pid } => {
            eprintln!("VOICEVOX daemon is already running (PID: {pid})");
            eprintln!("   Use 'voicevox-daemon --stop' to stop it.");
        }
        DaemonError::SocketPermissionDenied { path } => {
            eprintln!("Permission denied: Socket file is owned by another user");
            eprintln!("   Socket path: {}", path.display());
            eprintln!("   Please remove the file manually and try again.");
        }
        _ => eprintln!("{error}"),
    }
    startup_error_exit_code(error)
}

fn print_daemon_start_banner(socket_path: &Path) {
    println!("VOICEVOX Daemon v{}", env!("CARGO_PKG_VERSION"));
    println!("Starting user daemon...");
    println!("Socket: {} (user-specific)", socket_path.display());
    println!("Models: Load and unload per request (no caching)");
}

async fn daemon_is_responsive(socket_path: &Path) -> bool {
    UnixStream::connect(socket_path).await.is_ok()
}

fn print_socket_path_line(socket_path: &Path) {
    println!("Socket: {}", socket_path.display());
}

fn print_socket_not_running(socket_path: &Path) {
    println!("Daemon is not running");
    println!("   Socket: {}", socket_path.display());
}

fn print_pid_memory_info(pid_num: u32) {
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
        println!("Memory Info: {line}");
    }
}

async fn handle_stop_daemon(socket_path: &Path) -> Result<()> {
    println!("Stopping VOICEVOX daemon...");

    if !daemon_is_responsive(socket_path).await {
        print_socket_not_running(socket_path);
        return Ok(());
    }

    let pids = match crate::daemon::process::find_daemon_processes() {
        Ok(pids) => pids,
        Err(error) => {
            println!("Failed to find daemon process: {error}");
            println!("   Try manual: pkill -f -u $(id -u) voicevox-daemon");
            return Ok(());
        }
    };

    if pids.is_empty() {
        println!("No daemon process found");
        return Ok(());
    }

    for pid_num in pids {
        stop_daemon_process(pid_num, socket_path).await;
    }

    Ok(())
}

async fn stop_daemon_process(pid: u32, socket_path: &Path) {
    let kill_result = std::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status();

    match kill_result {
        Ok(status) if status.success() => {
            println!("Daemon stopped (PID: {pid})");
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            if daemon_is_responsive(socket_path).await {
                println!("Daemon may still be running");
            } else {
                println!("Socket cleanup confirmed");
            }
        }
        _ => {
            println!("Failed to stop daemon (PID: {pid})");
            println!("   Try: kill -9 {pid}");
        }
    }
}

async fn handle_status_daemon(socket_path: &Path) -> Result<()> {
    println!("VOICEVOX Daemon Status");
    println!("========================");

    if daemon_is_responsive(socket_path).await {
        println!("Status:  Running and responsive");
        print_socket_path_line(socket_path);

        if let Ok(pids) = crate::daemon::process::find_daemon_processes() {
            for pid_num in pids {
                println!("Process ID: {pid_num}");
                print_pid_memory_info(pid_num);
            }
        }
    } else {
        println!("Status:  Not running");
        print_socket_path_line(socket_path);
    }

    Ok(())
}

/// Executes daemon CLI flow from already-parsed flags and exits the process when required.
///
/// # Errors
///
/// Returns an error if command dispatch or daemon runtime fails.
pub async fn run_daemon_cli(socket_path: PathBuf, flags: DaemonRunFlags) -> Result<()> {
    if maybe_handle_control_commands(&socket_path, flags).await? {
        return Ok(());
    }

    if let ExecutionDecision::Exit(code) = maybe_detach(&socket_path, flags).await {
        std::process::exit(code);
    }

    if let Err(error) = ensure_startup_preconditions(&socket_path).await {
        std::process::exit(report_startup_error(&error));
    }

    print_daemon_start_banner(&socket_path);
    crate::daemon::run_daemon(socket_path, flags.start_mode.is_foreground()).await
}
