pub mod daemon_control;
pub mod inspect;
pub mod mcp_server;
pub mod output;
pub mod say;
pub mod synthesis_job;

pub use daemon_control::{run_daemon_cli, ControlCommand, DaemonRunFlags, StartMode};
pub use inspect::{run_list_models_command, run_list_speakers_command, run_status_command};
pub use mcp_server::run_mcp_server_app;
pub use output::{AppOutput, StdAppOutput};
pub use say::{run_say_synthesis, SaySynthesisRequest};
pub use synthesis_job::{
    connect_daemon_client_auto_start, synthesize_bytes_via_daemon, validate_text_synthesis_request,
    DaemonSynthesisBytesRequest, NoopAppOutput,
};
