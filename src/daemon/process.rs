use crate::daemon::{DaemonError, DaemonResult};
use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::process;

pub async fn check_and_prevent_duplicate(socket_path: &PathBuf) -> DaemonResult<()> {
    if socket_path.exists() {
        handle_existing_socket(socket_path).await?;
    }
    check_for_other_daemons()?;

    Ok(())
}

async fn handle_existing_socket(socket_path: &PathBuf) -> DaemonResult<()> {
    match tokio::net::UnixStream::connect(socket_path).await {
        Ok(_) => {
            let pids = find_daemon_processes().unwrap_or_default();
            let pid = pids.first().copied().unwrap_or(0);
            Err(DaemonError::AlreadyRunning { pid })
        }
        Err(_) => remove_stale_socket(socket_path),
    }
}

fn remove_stale_socket(socket_path: &PathBuf) -> DaemonResult<()> {
    println!("Removing stale socket file: {}", socket_path.display());

    fs::remove_file(socket_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => DaemonError::SocketPermissionDenied {
            path: socket_path.clone(),
        },
        _ => DaemonError::StartupFailed {
            message: format!("Failed to remove stale socket: {}", e),
        },
    })
}

fn check_for_other_daemons() -> DaemonResult<()> {
    let output = process::Command::new("pgrep")
        .arg("-x")
        .arg("-u")
        .arg(format!("{}", unsafe { libc::getuid() }))
        .arg("voicevox-daemon")
        .output();

    match output {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {
            check_pgrep_output(&output.stdout)
        }
        Ok(_) => Ok(()), // No processes found or empty output
        Err(_) => {
            println!("Could not check for existing processes (pgrep not available)");
            Ok(())
        }
    }
}

fn check_pgrep_output(stdout: &[u8]) -> DaemonResult<()> {
    let pids = String::from_utf8_lossy(stdout);
    let current_pid = process::id();

    let other_pids: Vec<u32> = pids
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|pid| pid.trim().parse::<u32>().ok())
        .filter(|&pid| pid != current_pid)
        .collect();

    match other_pids.first() {
        Some(&pid) => Err(DaemonError::AlreadyRunning { pid }),
        None => Ok(()),
    }
}

pub fn find_daemon_processes() -> Result<Vec<u32>> {
    let output = process::Command::new("pgrep")
        .arg("-f")
        .arg("-u")
        .arg(unsafe { libc::getuid() }.to_string())
        .arg("voicevox-daemon")
        .output();

    match output {
        Ok(output) if output.status.success() && !output.stdout.is_empty() => {
            parse_daemon_pids(&output.stdout)
        }
        _ => Ok(vec![]),
    }
}

fn parse_daemon_pids(stdout: &[u8]) -> Result<Vec<u32>> {
    let pids = String::from_utf8_lossy(stdout);
    let current_pid = process::id();

    let pids: Vec<u32> = pids
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|pid| pid.trim().parse::<u32>().ok())
        .filter(|&pid| pid != current_pid)
        .collect();

    Ok(pids)
}
