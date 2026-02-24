use std::time::Duration;

use crate::daemon::{self, EnsureDaemonRunningOptions};

#[derive(Debug, Clone, Copy)]
pub struct DaemonConnectRetryPolicy {
    pub attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for DaemonConnectRetryPolicy {
    fn default() -> Self {
        Self {
            attempts: daemon::startup::MAX_CONNECT_ATTEMPTS,
            initial_delay: daemon::startup::initial_retry_delay(),
            max_delay: daemon::startup::max_retry_delay(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DaemonAutoStartPolicy {
    pub startup_grace_period: Duration,
    pub final_connection_timeout: Duration,
    pub ensure_running: EnsureDaemonRunningOptions,
}

impl DaemonAutoStartPolicy {
    #[must_use]
    pub fn cli_default() -> Self {
        Self {
            startup_grace_period: Duration::from_millis(1000),
            final_connection_timeout: Duration::from_secs(5),
            ensure_running: EnsureDaemonRunningOptions {
                connect_timeout: super::transport::DAEMON_CONNECTION_TIMEOUT,
                wait_attempts: 20,
                initial_retry_delay: Duration::from_millis(500),
                max_retry_delay: Duration::from_secs(4),
                ..EnsureDaemonRunningOptions::default()
            },
        }
    }

    #[must_use]
    pub fn mcp_default() -> EnsureDaemonRunningOptions {
        EnsureDaemonRunningOptions {
            remove_stale_socket: true,
            connect_timeout: daemon::startup::connect_timeout(),
            wait_attempts: daemon::startup::MAX_CONNECT_ATTEMPTS,
            initial_retry_delay: daemon::startup::initial_retry_delay(),
            max_retry_delay: daemon::startup::max_retry_delay(),
            sleep_before_first_check: false,
        }
    }
}
