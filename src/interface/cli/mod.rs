pub mod audio;
pub mod daemon_cli;
pub mod daemon_rpc;
pub mod download;
pub mod input;
pub mod inspect;
pub mod playback;
pub mod say;
pub mod synthesis;

pub use crate::domain::daemon_control::{DaemonCliFlags, DaemonControlCommand, DaemonStartMode};
pub use crate::domain::synthesis::{TextSegmenter, TextSplitter};
pub use audio::play_audio_from_memory;
pub use daemon_cli::run_daemon_cli;
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
pub use playback::{emit_and_play, PlaybackOutcome, PlaybackRequest};
pub use say::{run_say_synthesis, SaySynthesisRequest};
pub use synthesis::{
    connect_daemon_rpc_auto_start, request_daemon_synthesis_bytes,
    request_streaming_synthesis_segments, select_synthesis_mode, select_synthesis_mode_with_config,
    stream_synthesis_to_sink, synthesize_bytes_via_daemon, validate_text_synthesis_request,
    DaemonSynthesisBytesRequest, NoopAppOutput, StreamingSynthesizer, SynthesisMode,
};
