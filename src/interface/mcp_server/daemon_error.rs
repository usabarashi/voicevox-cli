use crate::infrastructure::daemon::client::find_daemon_client_error;
use crate::infrastructure::ipc::DaemonErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VoiceTargetState {
    Unknown,
    Exists,
    Missing,
}

fn infer_voice_target_state(error: &anyhow::Error) -> VoiceTargetState {
    let Some(daemon_error): Option<&crate::infrastructure::daemon::client::DaemonClientError> =
        find_daemon_client_error(error)
    else {
        return VoiceTargetState::Unknown;
    };

    match daemon_error.code() {
        DaemonErrorCode::InvalidTargetId | DaemonErrorCode::ModelLoadFailed => {
            VoiceTargetState::Missing
        }
        DaemonErrorCode::SynthesisFailed | DaemonErrorCode::Internal => VoiceTargetState::Exists,
    }
}

pub fn format_daemon_client_error_for_mcp(error: &anyhow::Error) -> String {
    let Some(daemon_error): Option<&crate::infrastructure::daemon::client::DaemonClientError> =
        find_daemon_client_error(error)
    else {
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

#[must_use]
pub fn is_retryable_daemon_synthesis_error(error: &anyhow::Error) -> bool {
    !matches!(infer_voice_target_state(error), VoiceTargetState::Missing)
}
