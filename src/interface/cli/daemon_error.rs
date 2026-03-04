use crate::infrastructure::daemon::client::find_daemon_client_error;
use crate::infrastructure::ipc::DaemonErrorCode;

pub fn format_daemon_client_error_for_cli(error: &anyhow::Error) -> String {
    let Some(daemon_error): Option<&crate::infrastructure::daemon::client::DaemonClientError> =
        find_daemon_client_error(error)
    else {
        return format!("Synthesis request failed: {error}");
    };

    match daemon_error.code() {
        DaemonErrorCode::InvalidTargetId => {
            format!("Invalid style/model ID. {}", daemon_error.message())
        }
        DaemonErrorCode::ModelLoadFailed => {
            format!("Failed to load VOICEVOX model. {}", daemon_error.message())
        }
        DaemonErrorCode::SynthesisFailed => {
            format!("VOICEVOX synthesis failed. {}", daemon_error.message())
        }
        DaemonErrorCode::Internal => {
            format!("VOICEVOX daemon internal error. {}", daemon_error.message())
        }
    }
}

#[must_use]
pub fn daemon_client_exit_code(error: &anyhow::Error) -> Option<u8> {
    let daemon_error: &crate::infrastructure::daemon::client::DaemonClientError =
        find_daemon_client_error(error)?;
    Some(match daemon_error.code() {
        DaemonErrorCode::InvalidTargetId => 2,
        DaemonErrorCode::ModelLoadFailed => 3,
        DaemonErrorCode::SynthesisFailed => 4,
        DaemonErrorCode::Internal => 5,
    })
}
