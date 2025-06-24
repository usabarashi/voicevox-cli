pub mod server;
pub mod process;

pub use server::{DaemonState, handle_client, run_daemon};
pub use process::check_and_prevent_duplicate;