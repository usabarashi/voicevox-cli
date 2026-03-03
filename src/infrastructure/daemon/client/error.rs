use anyhow::anyhow;
use crate::infrastructure::ipc::DaemonErrorCode;

#[derive(Debug, thiserror::Error)]
#[error("{context}: {message}")]
pub struct DaemonClientError {
    context: String,
    code: DaemonErrorCode,
    message: String,
}

impl DaemonClientError {
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
    anyhow!(DaemonClientError::new(context, code, message))
}

pub fn find_daemon_client_error(error: &anyhow::Error) -> Option<&DaemonClientError> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<DaemonClientError>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_client_error_is_discoverable_through_anyhow_chain() {
        let err = daemon_response_error(
            "Synthesis error",
            DaemonErrorCode::InvalidTargetId,
            "bad id",
        );
        let wrapped = err.context("top level");

        let daemon_err = find_daemon_client_error(&wrapped).expect("daemon rpc error in chain");
        assert_eq!(daemon_err.code(), DaemonErrorCode::InvalidTargetId);
        assert_eq!(daemon_err.message(), "bad id");
    }
}
