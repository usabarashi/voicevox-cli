pub mod audio;
pub mod daemon_control;
pub mod daemon_rpc;
pub mod download;
pub mod input;
pub mod inspect;
pub mod say;
pub mod synthesis_job;

pub use audio::play_audio_from_memory;
pub use daemon_control::{run_daemon_cli, ControlCommand, DaemonRunFlags, StartMode};
pub use daemon_rpc::{
    daemon_mode, daemon_rpc_exit_code, find_daemon_rpc_error, format_daemon_rpc_error_for_cli,
    format_daemon_rpc_error_for_mcp, infer_voice_target_state, list_speakers_daemon,
    DaemonAutoStartPolicy, DaemonRpcClient, VoiceTargetState,
};
pub use download::{
    cleanup_unnecessary_files, count_vvm_files_recursive, ensure_models_available,
    has_startup_resources, launch_downloader_for_user, missing_startup_resources,
};
pub use input::get_input_text_from_sources;
pub use inspect::{run_list_models_command, run_list_speakers_command, run_status_command};
pub use say::{run_say_synthesis, SaySynthesisRequest};
pub use synthesis_job::{
    connect_daemon_rpc_auto_start, synthesize_bytes_via_daemon, validate_text_synthesis_request,
    DaemonSynthesisBytesRequest, NoopAppOutput,
};
