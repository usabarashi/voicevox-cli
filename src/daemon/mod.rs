pub mod process;
pub mod server;

use std::io;
use std::path::PathBuf;
use thiserror::Error;

pub use process::{check_and_prevent_duplicate, find_daemon_processes};
pub use server::{handle_client, run_daemon, DaemonState};

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

    #[error("No VOICEVOX models found. Run voicevox-setup to install.")]
    NoModelsAvailable,

    #[error("Failed to find daemon binary")]
    DaemonBinaryNotFound,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type for daemon operations
pub type DaemonResult<T> = Result<T, DaemonError>;

/// Exit codes for daemon operations
/// These codes are used to communicate specific error conditions
/// between processes without relying on stderr string parsing.
pub mod exit_codes {
    /// Daemon started successfully or was already running and responsive
    pub const SUCCESS: i32 = 0;

    /// General failure
    pub const FAILURE: i32 = 1;

    /// Daemon is already running (another instance exists)
    pub const ALREADY_RUNNING: i32 = 2;

    /// Permission denied (socket file owned by another user)
    pub const PERMISSION_DENIED: i32 = 3;

    /// No models available
    pub const NO_MODELS: i32 = 4;

    /// Daemon binary not found
    pub const BINARY_NOT_FOUND: i32 = 5;
}

/// Daemon startup constants
pub mod startup {
    use std::time::Duration;

    /// Maximum number of connection attempts when starting daemon
    pub const MAX_CONNECT_ATTEMPTS: u32 = 10;

    /// Initial delay between connection attempts (milliseconds)
    pub const INITIAL_RETRY_DELAY_MS: u64 = 100;

    /// Maximum delay between connection attempts (milliseconds)
    pub const MAX_RETRY_DELAY_MS: u64 = 1000;

    /// Initial connection timeout (seconds)
    pub const CONNECT_TIMEOUT_SECS: u64 = 1;

    /// Number of retries when daemon claims to be already running
    pub const ALREADY_RUNNING_RETRIES: u32 = 3;

    /// Get initial retry delay as Duration
    pub fn initial_retry_delay() -> Duration {
        Duration::from_millis(INITIAL_RETRY_DELAY_MS)
    }

    /// Get max retry delay as Duration
    pub fn max_retry_delay() -> Duration {
        Duration::from_millis(MAX_RETRY_DELAY_MS)
    }

    /// Get connection timeout as Duration
    pub fn connect_timeout() -> Duration {
        Duration::from_secs(CONNECT_TIMEOUT_SECS)
    }
}
