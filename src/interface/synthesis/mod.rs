pub mod daemon;
pub mod flow;
pub mod mode;
pub mod streaming;

pub use daemon::DaemonSynthesizer;
pub use flow::{
    connect_daemon_client_auto_start, synthesize_bytes_via_daemon, validate_text_synthesis_request,
    DaemonSynthesisBytesRequest, NoopAppOutput,
};
pub use mode::{select_synthesis_mode, select_synthesis_mode_with_config, SynthesisMode};
pub use streaming::StreamingSynthesizer;
