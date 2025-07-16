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
    #[arg(short, long)]
    version: bool,
}

async fn ensure_daemon_running() -> Result<()> {
    let socket_path = get_socket_path();

    match tokio::net::UnixStream::connect(&socket_path).await {
        Ok(_) => Ok(()),
        Err(_) => {
            let current_exe = std::env::current_exe()?;
            let daemon_path = current_exe
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Failed to get executable directory"))?
                .join("voicevox-daemon");

            if let Ok(output) = Command::new(&daemon_path).arg("--start").output() {
                if output.status.success() {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
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

    let _ = ensure_daemon_running().await;
    voicevox_cli::mcp::run_mcp_server().await?;

    Ok(())
}
