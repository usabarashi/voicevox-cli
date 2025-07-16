use anyhow::Result;
use clap::Parser;
use std::process::Command;
use voicevox_cli::paths::get_socket_path;

#[derive(Parser, Debug)]
#[command(
    name = "voicevox-mcp-server",
    about = "VOICEVOX MCP Server for AI assistants",
    version
)]
struct Args {
    /// Show version information
    #[arg(short, long)]
    version: bool,
}

/// Ensure daemon is running (start if not)
async fn ensure_daemon_running() -> Result<()> {
    let socket_path = get_socket_path();

    match tokio::net::UnixStream::connect(&socket_path).await {
        Ok(_) => {
            eprintln!("VOICEVOX daemon is already running");
            Ok(())
        }
        Err(_) => {
            eprintln!("Starting VOICEVOX daemon...");

            let current_exe = std::env::current_exe()?;
            let daemon_path = current_exe
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Failed to get executable directory"))?
                .join("voicevox-daemon");

            match Command::new(&daemon_path).arg("--start").output() {
                Ok(output) => {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if stderr.contains("Operation not permitted") {
                            eprintln!("Note: Cannot auto-start daemon due to system restrictions");
                            eprintln!("Please start the daemon manually: voicevox-daemon --start");
                        } else {
                            eprintln!("Failed to start daemon: {}", stderr.trim());
                        }
                    } else {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        eprintln!("VOICEVOX daemon started successfully");
                    }
                }
                Err(e) => {
                    eprintln!("Could not execute daemon: {e}");
                    eprintln!("Please ensure voicevox-daemon is in your PATH");
                }
            }

            Ok(())
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.version {
        println!("voicevox-mcp-server {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Ensure daemon is running
    if let Err(e) = ensure_daemon_running().await {
        eprintln!("Warning: Could not ensure daemon is running: {e}");
        eprintln!("MCP server will start anyway, but synthesis may fail");
    }

    // Run MCP server
    voicevox_cli::mcp::run_mcp_server().await?;

    Ok(())
}
