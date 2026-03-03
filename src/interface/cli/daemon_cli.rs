use anyhow::Result;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::Duration;

use crate::domain::daemon_control::{
    daemon_not_running_lines, daemon_socket_line, daemon_start_banner_lines, daemon_usage_lines,
    decide_daemon_invocation, DaemonCliFlags, DaemonInvocation,
};
use crate::infrastructure::daemon::{
    check_and_prevent_duplicate, exit_codes as exit_daemon, is_socket_responsive,
    pid_memory_info_line, terminate_process, DaemonError,
};
use crate::interface::{AppOutput, StdAppOutput};

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
        is_socket_responsive(socket_path)
    }

    fn find_daemon_processes(&self) -> anyhow::Result<Vec<u32>> {
        crate::infrastructure::daemon::process::find_daemon_processes()
    }

    fn pid_memory_info_line(&self, pid_num: u32) -> Option<String> {
        pid_memory_info_line(pid_num)
    }

    fn kill_term(&self, pid: u32) -> bool {
        terminate_process(pid)
    }
}

fn print_usage_banner(output: &dyn AppOutput) {
    for line in daemon_usage_lines(env!("CARGO_PKG_VERSION")) {
        output.info(&line);
    }
}

async fn maybe_handle_control_commands(
    socket_path: &Path,
    flags: DaemonCliFlags,
    output: &dyn AppOutput,
) -> Result<bool> {
    match decide_daemon_invocation(flags) {
        DaemonInvocation::Stop => {
            handle_stop_daemon(socket_path, output).await?;
            Ok(true)
        }
        DaemonInvocation::Status => {
            handle_status_daemon(socket_path, output).await?;
            Ok(true)
        }
        DaemonInvocation::Restart => {
            output.info("Restarting daemon...");
            let _ = handle_stop_daemon(socket_path, output).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
            Ok(false)
        }
        DaemonInvocation::ShowUsage => {
            print_usage_banner(output);
            Ok(true)
        }
        DaemonInvocation::Start => Ok(false),
    }
}

async fn maybe_detach(
    socket_path: &Path,
    flags: DaemonCliFlags,
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
        .env("VOICEVOX_DETACH_PARENT_PID", std::process::id().to_string())
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
                    if status.code() == Some(exit_daemon::ALREADY_RUNNING) {
                        output.info("VOICEVOX daemon is already running");
                        return ExecutionDecision::exit(exit_daemon::ALREADY_RUNNING);
                    }
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
    for line in daemon_start_banner_lines(env!("CARGO_PKG_VERSION"), socket_path) {
        output.info(&line);
    }
}

fn print_socket_path_line(socket_path: &Path, output: &dyn AppOutput) {
    output.info(&daemon_socket_line(socket_path));
}

fn print_socket_not_running(socket_path: &Path, output: &dyn AppOutput) {
    for line in daemon_not_running_lines(socket_path) {
        output.info(&line);
    }
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
    let responsive = os.is_responsive(socket_path);

    let pids = match os.find_daemon_processes() {
        Ok(pids) => pids,
        Err(error) => {
            output.info(&format!("Failed to find daemon process: {error}"));
            output.info("   Try manual: pkill -f -u $(id -u) voicevox-daemon");
            return Ok(());
        }
    };

    if !responsive {
        print_socket_not_running(socket_path, output);
        if !pids.is_empty() {
            output.info("Found daemon process(es) without responsive socket. Stopping anyway...");
        }
    }

    if pids.is_empty() {
        if responsive {
            output.info("No daemon process found");
        }
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
pub async fn run_daemon_cli(socket_path: PathBuf, flags: DaemonCliFlags) -> Result<()> {
    let output = StdAppOutput;
    run_daemon_cli_with_output(socket_path, flags, &output).await
}

pub async fn run_daemon_cli_with_output(
    socket_path: PathBuf,
    flags: DaemonCliFlags,
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
    crate::infrastructure::daemon::run_daemon(socket_path, flags.start_mode.is_foreground()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interface::output::BufferAppOutput;
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

    #[tokio::test]
    async fn stop_kills_process_even_when_socket_is_not_responsive() {
        let output = BufferAppOutput::default();
        let os = FakeDaemonControlOs {
            responsive: Mutex::new(VecDeque::from([false, false])),
            pids: vec![25672],
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
        assert!(text.contains("Daemon is not running"));
        assert!(
            text.contains("Found daemon process(es) without responsive socket. Stopping anyway...")
        );
        assert!(text.contains("Daemon stopped (PID: 25672)"));
    }
}
