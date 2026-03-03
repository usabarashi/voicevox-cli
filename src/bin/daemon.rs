use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use voicevox_cli::infrastructure::paths::get_socket_path;
use voicevox_cli::interface::cli::{
    run_daemon_cli, DaemonCliFlags, DaemonControlCommand, DaemonStartMode,
};

// Clap option flags are intentionally represented as booleans.
#[allow(clippy::struct_excessive_bools)]
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

    fn to_daemon_flags(&self) -> DaemonCliFlags {
        DaemonCliFlags {
            start_mode: DaemonStartMode::from_flags(self.foreground, self.detach),
            mode_flag_explicit: self.foreground || self.detach,
            start: self.start,
            control: self.control_command(),
        }
    }

    fn control_command(&self) -> DaemonControlCommand {
        if self.stop {
            DaemonControlCommand::Stop
        } else if self.status {
            DaemonControlCommand::Status
        } else if self.restart {
            DaemonControlCommand::Restart
        } else {
            DaemonControlCommand::None
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = CliArgs::parse();
    run_daemon_cli(args.socket_path(), args.to_daemon_flags()).await
}
