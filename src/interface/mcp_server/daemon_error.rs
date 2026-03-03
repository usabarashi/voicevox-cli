use crate::infrastructure::daemon::rpc::find_daemon_rpc_error;
use crate::interface::ipc::DaemonErrorCode;

pub fn format_daemon_rpc_error_for_mcp(error: &anyhow::Error) -> String {
    let Some(daemon_error) = find_daemon_rpc_error(error) else {
        return format!("Failed to reach VOICEVOX daemon or synthesize audio: {error}");
    };

    match daemon_error.code() {
        DaemonErrorCode::InvalidTargetId => {
            format!("Invalid style/model ID: {}", daemon_error.message())
        }
        DaemonErrorCode::ModelLoadFailed => {
            format!("VOICEVOX model load failed: {}", daemon_error.message())
        }
        DaemonErrorCode::SynthesisFailed => {
            format!("VOICEVOX synthesis failed: {}", daemon_error.message())
        }
        DaemonErrorCode::Internal => {
            format!("VOICEVOX daemon internal error: {}", daemon_error.message())
        }
    }
}
