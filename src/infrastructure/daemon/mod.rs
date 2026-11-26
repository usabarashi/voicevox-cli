pub mod bootstrap;
pub mod client;
pub mod control;
pub mod process;
pub mod server;
pub mod socket_probe;
pub mod start_process;
pub mod state;

use std::io;
use std::path::PathBuf;
use thiserror::Error;

pub use bootstrap::{
    EnsureDaemonRunningOptions, EnsureDaemonRunningOutcome, ensure_daemon_running,
    recover_stuck_daemon_and_retry,
};
pub use control::{is_socket_responsive, pid_memory_info_line, terminate_process};
pub use process::{
    check_and_prevent_duplicate, find_daemon_processes, remove_stale_socket_if_present,
};
pub use server::run_daemon;
pub use start_process::{StartDaemonOutcome, find_daemon_binary, start_daemon_detached};
pub use state::DaemonState;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Daemon is already running (PID: {pid})")]
    AlreadyRunning { pid: u32 },

    #[error("Socket file exists but is owned by another user: {path}")]
    SocketPermissionDenied { path: PathBuf },

    #[error("Failed to start daemon: {message}")]
    StartupFailed { message: String },

    #[error("Daemon started but is not responding after {attempts} attempts")]
    NotResponding { attempts: u32 },

    #[error("Failed to connect to daemon: {0}")]
    ConnectionFailed(#[from] io::Error),

    #[error(
        "No VOICEVOX models found. Please run 'voicevox-setup' or place .vvm files in the models directory."
    )]
    NoModelsAvailable,

    #[error("Failed to find daemon binary")]
    DaemonBinaryNotFound,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type DaemonResult<T> = Result<T, DaemonError>;

/// These codes are used to communicate specific error conditions
/// between processes without relying on stderr string parsing.
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;

    pub const FAILURE: i32 = 1;

    pub const ALREADY_RUNNING: i32 = 2;

    pub const PERMISSION_DENIED: i32 = 3;

    pub const NO_MODELS: i32 = 4;

    pub const BINARY_NOT_FOUND: i32 = 5;
}

pub mod startup {
    use std::time::Duration;

    pub const MAX_CONNECT_ATTEMPTS: u32 = 10;

    pub const INITIAL_RETRY_DELAY_MS: u64 = 100;

    pub const MAX_RETRY_DELAY_MS: u64 = 1000;

    pub const CONNECT_TIMEOUT_SECS: u64 = 1;

    pub const ALREADY_RUNNING_RETRIES: u32 = 3;

    #[must_use]
    pub const fn initial_retry_delay() -> Duration {
        Duration::from_millis(INITIAL_RETRY_DELAY_MS)
    }

    #[must_use]
    pub const fn max_retry_delay() -> Duration {
        Duration::from_millis(MAX_RETRY_DELAY_MS)
    }

    #[must_use]
    pub const fn connect_timeout() -> Duration {
        Duration::from_secs(CONNECT_TIMEOUT_SECS)
    }
}
