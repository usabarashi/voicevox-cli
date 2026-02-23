use anyhow::Result;
use clap::Parser;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use tokio::net::UnixStream;
use voicevox_cli::daemon::{check_and_prevent_duplicate, exit_codes as exit_daemon, DaemonError};
use voicevox_cli::paths::get_socket_path;

#[derive(Debug, Parser)]
#[command(
    name = "voicevox-daemon",
    version,
    about = "VOICEVOX Daemon - Background TTS service with pre-loaded models"
)]
struct CliArgs {
    #[arg(long = "socket-path", short = 's', value_name = "PATH")]
    socket_path: Option<PathBuf>,

    #[arg(long, short = 'f')]
    foreground: bool,

    #[arg(long, short = 'd')]
    detach: bool,

    #[arg(long, help = "Start the daemon (default behavior)")]
    start: bool,

    #[arg(long, conflicts_with_all = ["status", "restart"])]
    stop: bool,

    #[arg(long, conflicts_with_all = ["stop", "restart"])]
    status: bool,

    #[arg(long, conflicts_with_all = ["stop", "status"])]
    restart: bool,
}

impl CliArgs {
    fn socket_path(&self) -> PathBuf {
        self.socket_path.clone().unwrap_or_else(get_socket_path)
    }

    fn to_daemon_flags(&self) -> DaemonFlags {
        DaemonFlags {
            start_mode: StartMode::from_flags(self.foreground, self.detach),
            mode_flag_explicit: self.foreground || self.detach,
            start: self.start,
            control: self.control_command(),
        }
    }

    fn control_command(&self) -> ControlCommand {
        if self.stop {
            ControlCommand::Stop
        } else if self.status {
            ControlCommand::Status
        } else if self.restart {
            ControlCommand::Restart
        } else {
            ControlCommand::None
        }
    }
}

#[derive(Clone, Copy)]
enum StartMode {
    Foreground,
    Detached,
}

impl StartMode {
    fn from_flags(foreground: bool, detach: bool) -> Self {
        if detach && !foreground {
            Self::Detached
        } else {
            Self::Foreground
        }
    }

    const fn is_foreground(self) -> bool {
        matches!(self, Self::Foreground)
    }

    const fn should_detach(self) -> bool {
        matches!(self, Self::Detached)
    }
}

#[derive(Clone, Copy)]
enum ControlCommand {
    None,
    Stop,
    Status,
    Restart,
}

#[derive(Clone, Copy)]
struct DaemonFlags {
    start_mode: StartMode,
    mode_flag_explicit: bool,
    start: bool,
    control: ControlCommand,
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

async fn maybe_handle_control_commands(socket_path: &Path, flags: DaemonFlags) -> Result<bool> {
    match flags.control {
        ControlCommand::Stop => {
            handle_stop_daemon(socket_path).await?;
            return Ok(true);
        }
        ControlCommand::Status => {
            handle_status_daemon(socket_path).await?;
            return Ok(true);
        }
        ControlCommand::Restart => {
            println!("Restarting daemon...");
            let _ = handle_stop_daemon(socket_path).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
        }
        ControlCommand::None => {}
    }
    if !flags.start && matches!(flags.control, ControlCommand::None) && !flags.mode_flag_explicit {
        print_usage_banner();
        return Ok(true);
    }
    Ok(false)
}

async fn maybe_detach(socket_path: &Path, flags: DaemonFlags) -> ExecutionDecision {
    if !flags.start_mode.should_detach() {
        return ExecutionDecision::Continue;
    }

    println!("Starting daemon in detached mode...");

    let mut args: Vec<String> = std::env::args().collect();
    args.retain(|arg| arg != "--detach" && arg != "-d");
    args.push("--foreground".to_string());

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
                Err(e) => {
                    eprintln!("Failed to check daemon status: {e}");
                    ExecutionDecision::exit(1)
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to spawn daemon process: {e}");
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
        _ => {
            eprintln!("{error}");
        }
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();
    let socket_path = args.socket_path();
    let flags = args.to_daemon_flags();

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
    voicevox_cli::daemon::run_daemon(socket_path, flags.start_mode.is_foreground()).await
}

async fn handle_stop_daemon(socket_path: &Path) -> Result<()> {
    println!("Stopping VOICEVOX daemon...");

    if !daemon_is_responsive(socket_path).await {
        print_socket_not_running(socket_path);
        return Ok(());
    }

    let pids = match voicevox_cli::daemon::process::find_daemon_processes() {
        Ok(pids) => pids,
        Err(e) => {
            println!("Failed to find daemon process: {e}");
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

        if let Ok(pids) = voicevox_cli::daemon::process::find_daemon_processes() {
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
