pub mod audio;
pub mod daemon_client;
pub mod download;
pub mod input;

pub use audio::{emit_synthesized_audio, play_audio_from_memory};
pub use daemon_client::{
    daemon_mode, daemon_rpc_exit_code, find_daemon_rpc_error, format_daemon_rpc_error_for_cli,
    format_daemon_rpc_error_for_mcp, list_speakers_daemon, DaemonAutoStartPolicy, DaemonClient,
};
pub use download::{
    cleanup_unnecessary_files, count_vvm_files_recursive, ensure_models_available,
    launch_downloader_for_user,
};
pub use input::get_input_text_from_sources;
