use anyhow::{anyhow, Result};
use std::fs;
use std::path::PathBuf;
use std::process;

pub async fn check_and_prevent_duplicate(socket_path: &PathBuf) -> Result<()> {
    if socket_path.exists() {
        // Try to connect to existing daemon
        match tokio::net::UnixStream::connect(socket_path).await {
            Ok(_) => {
                return Err(anyhow!(
                    "VOICEVOX daemon is already running at {}. Use 'pkill -f -u {} voicevox-daemon' to stop it.",
                    socket_path.display(), unsafe { libc::getuid() }
                ));
            }
            Err(_) => {
                println!("Removing stale socket file: {}", socket_path.display());
                if let Err(e) = fs::remove_file(socket_path) {
                    return Err(anyhow!("Failed to remove stale socket: {}", e));
                }
            }
        }
    }

    match process::Command::new("pgrep")
        .arg("-x")
        .arg("-u")
        .arg(format!("{}", unsafe { libc::getuid() }))
        .arg("voicevox-daemon")
        .output()
    {
        Ok(output) => {
            if output.status.success() && !output.stdout.is_empty() {
                let pids = String::from_utf8_lossy(&output.stdout);
                let current_pid = process::id();
                let other_pids: Vec<&str> = pids
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .filter(|pid| pid.trim().parse::<u32>().unwrap_or(0) != current_pid)
                    .collect();

                if !other_pids.is_empty() {
                    return Err(anyhow!(
                        "VOICEVOX daemon process(es) already running for this user (PIDs: {}). Stop them first with 'voicevox-daemon --stop'",
                        other_pids.join(", ")
                    ));
                }
            }
        }
        Err(_) => {
            // pgrep not available, continue anyway
            println!("Could not check for existing processes (pgrep not available)");
        }
    }

    Ok(())
}

/// Find daemon processes for the current user
pub fn find_daemon_processes() -> Result<Vec<u32>> {
    match process::Command::new("pgrep")
        .arg("-f")
        .arg("-u")
        .arg(unsafe { libc::getuid() }.to_string())
        .arg("voicevox-daemon")
        .output()
    {
        Ok(output) => {
            if output.status.success() && !output.stdout.is_empty() {
                let pids = String::from_utf8_lossy(&output.stdout);
                let current_pid = process::id();

                let pids: Vec<u32> = pids
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .filter_map(|pid| pid.trim().parse::<u32>().ok())
                    .filter(|&pid| pid != current_pid)
                    .collect();

                Ok(pids)
            } else {
                Ok(vec![])
            }
        }
        Err(_) => Ok(vec![]), // pgrep not available
    }
}
