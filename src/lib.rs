// Re-export all modules for backward compatibility
pub mod bindings;
pub mod core;
pub mod ipc;
pub mod voice;
pub mod paths;
pub mod setup;
pub mod client;
pub mod daemon;

// Re-export commonly used types and functions for backward compatibility
pub use core::VoicevoxCore;
pub use ipc::{DaemonRequest, DaemonResponse, SynthesizeOptions};
pub use voice::{Speaker, Style, get_model_for_voice_id, get_voice_mapping, resolve_voice_name};
pub use paths::{get_socket_path, find_models_dir, find_models_dir_client, find_openjtalk_dict};
pub use setup::{attempt_first_run_setup, is_valid_models_directory};

// Re-export bindings
pub use bindings::*;