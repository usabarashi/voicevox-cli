pub mod daemon_synthesis;
pub mod streaming_synthesis;
pub mod synthesis_flow;
pub mod synthesis_mode;

pub use daemon_synthesis::{
    request_daemon_synthesis_bytes, request_streaming_synthesis_segments, stream_synthesis_to_sink,
};
pub use streaming_synthesis::StreamingSynthesizer;
pub use synthesis_flow::{
    connect_daemon_rpc_auto_start, synthesize_bytes_via_daemon, validate_text_synthesis_request,
    DaemonSynthesisBytesRequest, NoopAppOutput,
};
pub use synthesis_mode::{select_synthesis_mode, select_synthesis_mode_with_config, SynthesisMode};
