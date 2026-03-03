use anyhow::Result;
use clap::Parser;

use voicevox_cli::interface::mcp_server::run_mcp_server_app;

#[derive(Parser, Debug)]
#[command(
    name = "voicevox-mcp-server",
    about = "VOICEVOX MCP Server for AI assistants",
    version
)]
struct Args;

#[tokio::main]
async fn main() -> Result<()> {
    let _ = Args::parse();
    run_mcp_server_app().await
}
