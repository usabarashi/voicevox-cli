//! Server-side daemon functionality for voice synthesis
//!
//! This module implements the background daemon process that pre-loads voice models
//! and handles synthesis requests via Unix socket IPC. Designed for high performance
//! with instant response times after initial setup.

/// Background server implementation with model management
pub mod server;

/// Process management and duplicate prevention
pub mod process;

pub use server::{DaemonState, handle_client, run_daemon};
pub use process::check_and_prevent_duplicate;