use anyhow::Result;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::Duration;

use crate::app::{AppOutput, StdAppOutput};
use crate::daemon::{check_and_prevent_duplicate, exit_codes as exit_daemon, DaemonError};

fn allow_unsafe_path_commands() -> bool {
    std::env::var_os("VOICEVOX_ALLOW_UNSAFE_PATH_COMMANDS").is_some()
}

fn system_command_path(preferred: &'static str, fallback_name: &'static str) -> &'static str {
    if std::path::Path::new(preferred).is_file() {
        preferred
    } else if allow_unsafe_path_commands() {
        fallback_name
    } else {
        preferred
    }
}

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

trait DaemonControlOs {
    fn is_responsive(&self, socket_path: &Path) -> bool;
    fn find_daemon_processes(&self) -> anyhow::Result<Vec<u32>>;
    fn pid_memory_info_line(&self, pid_num: u32) -> Option<String>;
    fn kill_term(&self, pid: u32) -> bool;
}

struct SystemDaemonControlOs;

impl DaemonControlOs for SystemDaemonControlOs {
    fn is_responsive(&self, socket_path: &Path) -> bool {
        std::os::unix::net::UnixStream::connect(socket_path).is_ok()
    }

    fn find_daemon_processes(&self) -> anyhow::Result<Vec<u32>> {
        crate::daemon::process::find_daemon_processes()
    }

    fn pid_memory_info_line(&self, pid_num: u32) -> Option<String> {
        let ps_output = std::process::Command::new(system_command_path("/bin/ps", "ps"))
            .args(["-p", &pid_num.to_string(), "-o", "rss,pmem,time"])
            .output()
            .ok()?;

        if !ps_output.status.success() {
            return None;
        }

        String::from_utf8_lossy(&ps_output.stdout)
            .lines()
            .nth(1)
            .map(str::trim)
            .map(ToOwned::to_owned)
    }

    fn kill_term(&self, pid: u32) -> bool {
        std::process::Command::new(system_command_path("/bin/kill", "kill"))
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .is_ok_and(|status| status.success())
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

fn print_socket_path_line(socket_path: &Path, output: &dyn AppOutput) {
    output.info(&format!("Socket: {}", socket_path.display()));
}

fn print_socket_not_running(socket_path: &Path, output: &dyn AppOutput) {
    output.info("Daemon is not running");
    output.info(&format!("   Socket: {}", socket_path.display()));
}

fn print_pid_memory_info(pid_num: u32, output: &dyn AppOutput, os: &dyn DaemonControlOs) {
    if let Some(line) = os.pid_memory_info_line(pid_num) {
        output.info(&format!("Memory Info: {line}"));
    }
}

async fn handle_stop_daemon(socket_path: &Path, output: &dyn AppOutput) -> Result<()> {
    let os = SystemDaemonControlOs;
    handle_stop_daemon_with_os(socket_path, output, &os, Duration::from_millis(1000)).await
}

async fn handle_stop_daemon_with_os(
    socket_path: &Path,
    output: &dyn AppOutput,
    os: &dyn DaemonControlOs,
    post_kill_delay: Duration,
) -> Result<()> {
    output.info("Stopping VOICEVOX daemon...");

    if !os.is_responsive(socket_path) {
        print_socket_not_running(socket_path, output);
        return Ok(());
    }

    let pids = match os.find_daemon_processes() {
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
        stop_daemon_process_with_os(pid_num, socket_path, output, os, post_kill_delay).await;
    }

    Ok(())
}

async fn stop_daemon_process_with_os(
    pid: u32,
    socket_path: &Path,
    output: &dyn AppOutput,
    os: &dyn DaemonControlOs,
    post_kill_delay: Duration,
) {
    if os.kill_term(pid) {
        output.info(&format!("Daemon stopped (PID: {pid})"));
        tokio::time::sleep(post_kill_delay).await;

        if os.is_responsive(socket_path) {
            output.info("Daemon may still be running");
        } else {
            output.info("Socket cleanup confirmed");
        }
    } else {
        output.info(&format!("Failed to stop daemon (PID: {pid})"));
        output.info(&format!("   Try: kill -9 {pid}"));
    }
}

async fn handle_status_daemon(socket_path: &Path, output: &dyn AppOutput) -> Result<()> {
    let os = SystemDaemonControlOs;
    handle_status_daemon_with_os(socket_path, output, &os).await
}

async fn handle_status_daemon_with_os(
    socket_path: &Path,
    output: &dyn AppOutput,
    os: &dyn DaemonControlOs,
) -> Result<()> {
    output.info("VOICEVOX Daemon Status");
    output.info("========================");

    if os.is_responsive(socket_path) {
        output.info("Status:  Running and responsive");
        print_socket_path_line(socket_path, output);

        if let Ok(pids) = os.find_daemon_processes() {
            for pid_num in pids {
                output.info(&format!("Process ID: {pid_num}"));
                print_pid_memory_info(pid_num, output, os);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::output::BufferAppOutput;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FakeDaemonControlOs {
        responsive: Mutex<VecDeque<bool>>,
        pids: Vec<u32>,
        pids_error: Option<String>,
        memory_line: Option<String>,
        kill_ok: bool,
    }

    impl DaemonControlOs for FakeDaemonControlOs {
        fn is_responsive(&self, _socket_path: &Path) -> bool {
            self.responsive
                .lock()
                .expect("responsive lock")
                .pop_front()
                .unwrap_or(false)
        }

        fn find_daemon_processes(&self) -> anyhow::Result<Vec<u32>> {
            match &self.pids_error {
                Some(message) => Err(anyhow::anyhow!(message.clone())),
                None => Ok(self.pids.clone()),
            }
        }

        fn pid_memory_info_line(&self, _pid_num: u32) -> Option<String> {
            self.memory_line.clone()
        }

        fn kill_term(&self, _pid: u32) -> bool {
            self.kill_ok
        }
    }

    #[tokio::test]
    async fn status_uses_os_abstraction_for_pid_and_memory_output() {
        let output = BufferAppOutput::default();
        let os = FakeDaemonControlOs {
            responsive: Mutex::new(VecDeque::from([true])),
            pids: vec![1234],
            pids_error: None,
            memory_line: Some("20480 0.1 00:00:01".to_string()),
            kill_ok: false,
        };

        handle_status_daemon_with_os(Path::new("/tmp/test.sock"), &output, &os)
            .await
            .expect("status ok");

        let text = output.infos().join("\n");
        assert!(text.contains("Status:  Running and responsive"));
        assert!(text.contains("Process ID: 1234"));
        assert!(text.contains("Memory Info: 20480 0.1 00:00:01"));
    }

    #[tokio::test]
    async fn stop_reports_kill_failure_without_shelling_out() {
        let output = BufferAppOutput::default();
        let os = FakeDaemonControlOs {
            responsive: Mutex::new(VecDeque::from([true])),
            pids: vec![42],
            pids_error: None,
            memory_line: None,
            kill_ok: false,
        };

        handle_stop_daemon_with_os(
            Path::new("/tmp/test.sock"),
            &output,
            &os,
            Duration::from_millis(0),
        )
        .await
        .expect("stop ok");

        let text = output.infos().join("\n");
        assert!(text.contains("Stopping VOICEVOX daemon..."));
        assert!(text.contains("Failed to stop daemon (PID: 42)"));
        assert!(text.contains("Try: kill -9 42"));
    }

    #[tokio::test]
    async fn stop_reports_socket_cleanup_after_successful_kill() {
        let output = BufferAppOutput::default();
        let os = FakeDaemonControlOs {
            responsive: Mutex::new(VecDeque::from([true, false])),
            pids: vec![7],
            pids_error: None,
            memory_line: None,
            kill_ok: true,
        };

        handle_stop_daemon_with_os(
            Path::new("/tmp/test.sock"),
            &output,
            &os,
            Duration::from_millis(0),
        )
        .await
        .expect("stop ok");

        let text = output.infos().join("\n");
        assert!(text.contains("Daemon stopped (PID: 7)"));
        assert!(text.contains("Socket cleanup confirmed"));
    }
}
