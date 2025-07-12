//! Server-side daemon functionality for voice synthesis
//!
//! This module implements the background daemon process that pre-loads voice models
//! and handles synthesis requests via Unix socket IPC. Designed for high performance
//! with instant response times after initial setup.

/// Background server implementation with model management
pub mod server;

/// Process management and duplicate prevention
pub mod process;

/// Zero-copy audio streaming support
pub mod streaming;

/// File descriptor passing for zero-copy transfer
pub mod fd_passing;

/// FD passing server with stream reuse pattern
pub mod fd_server;

pub use process::check_and_prevent_duplicate;
pub use server::{handle_client, run_daemon, DaemonState};
pub use streaming::SharedAudioBuffer;
