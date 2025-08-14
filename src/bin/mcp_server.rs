use anyhow::Result;
use clap::Parser;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
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
    let connect_timeout = Duration::from_secs(5);

    match timeout(
        connect_timeout,
        tokio::net::UnixStream::connect(&socket_path),
    )
    .await
    {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(_)) | Err(_) => {
            let current_exe = std::env::current_exe()?;
            let daemon_path = current_exe
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Failed to get executable directory"))?
                .join("voicevox-daemon");

            let output = Command::new(&daemon_path).arg("--start").output().await?;
            if output.status.success() {
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(anyhow::anyhow!(
                    "Failed to start daemon. Stderr: {}",
                    stderr
                ))
            }
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

    ensure_daemon_running().await?;
    voicevox_cli::mcp::run_mcp_server().await?;

    Ok(())
}
