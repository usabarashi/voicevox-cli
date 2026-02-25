use anyhow::anyhow;

use crate::ipc::DaemonErrorCode;

#[derive(Debug, thiserror::Error)]
#[error("{context}: {message}")]
pub struct DaemonRpcError {
    context: String,
    code: DaemonErrorCode,
    message: String,
}

impl DaemonRpcError {
    fn new(context: &str, code: DaemonErrorCode, message: &str) -> Self {
        Self {
            context: context.to_owned(),
            code,
            message: message.to_owned(),
        }
    }

    #[must_use]
    pub const fn code(&self) -> DaemonErrorCode {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn daemon_response_error(context: &str, code: DaemonErrorCode, message: &str) -> anyhow::Error {
    anyhow!(DaemonRpcError::new(context, code, message))
}

pub fn find_daemon_rpc_error(error: &anyhow::Error) -> Option<&DaemonRpcError> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<DaemonRpcError>())
}

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

pub fn format_daemon_rpc_error_for_cli(error: &anyhow::Error) -> String {
    let Some(daemon_error) = find_daemon_rpc_error(error) else {
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
pub fn daemon_rpc_exit_code(error: &anyhow::Error) -> Option<u8> {
    let daemon_error = find_daemon_rpc_error(error)?;
    Some(match daemon_error.code() {
        DaemonErrorCode::InvalidTargetId => 2,
        DaemonErrorCode::ModelLoadFailed => 3,
        DaemonErrorCode::SynthesisFailed => 4,
        DaemonErrorCode::Internal => 5,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_rpc_error_is_discoverable_through_anyhow_chain() {
        let err = daemon_response_error(
            "Synthesis error",
            DaemonErrorCode::InvalidTargetId,
            "bad id",
        );
        let wrapped = err.context("top level");

        let daemon_err = find_daemon_rpc_error(&wrapped).expect("daemon rpc error in chain");
        assert_eq!(daemon_err.code(), DaemonErrorCode::InvalidTargetId);
        assert_eq!(daemon_err.message(), "bad id");
    }

    #[test]
    fn formats_structured_errors_for_mcp() {
        let err = daemon_response_error(
            "Synthesis error",
            DaemonErrorCode::ModelLoadFailed,
            "model 7 missing",
        );
        let wrapped = err.context("wrapper");

        let text = format_daemon_rpc_error_for_mcp(&wrapped);
        assert!(text.contains("model load failed"));
        assert!(text.contains("model 7 missing"));
    }

    #[test]
    fn formats_structured_errors_for_cli() {
        let err = daemon_response_error(
            "Synthesis error",
            DaemonErrorCode::SynthesisFailed,
            "core returned error",
        );
        let wrapped = err.context("wrapper");

        let text = format_daemon_rpc_error_for_cli(&wrapped);
        assert!(text.contains("VOICEVOX synthesis failed"));
        assert!(text.contains("core returned error"));
    }

    #[test]
    fn maps_daemon_error_codes_to_process_exit_codes() {
        let err = daemon_response_error(
            "Synthesis error",
            DaemonErrorCode::InvalidTargetId,
            "bad id",
        )
        .context("wrapper");
        assert_eq!(daemon_rpc_exit_code(&err), Some(2));
    }
}
