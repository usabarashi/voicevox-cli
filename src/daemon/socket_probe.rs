use std::path::Path;
use std::time::Duration;

use tokio::time::timeout;

pub async fn try_connect_with_timeout(socket_path: &Path, connect_timeout: Duration) -> bool {
    matches!(
        timeout(
            connect_timeout,
            tokio::net::UnixStream::connect(socket_path)
        )
        .await,
        Ok(Ok(_))
    )
}

pub async fn wait_for_socket_ready_with_backoff<F>(
    socket_path: &Path,
    attempts: u32,
    initial_delay: Duration,
    max_delay: Duration,
    sleep_before_first_check: bool,
    mut on_retry: F,
) -> bool
where
    F: FnMut(u32),
{
    let mut retry_delay = initial_delay;

    for attempt in 0..attempts {
        if sleep_before_first_check || attempt > 0 {
            on_retry(attempt);
            tokio::time::sleep(retry_delay).await;
        }

        if tokio::net::UnixStream::connect(socket_path).await.is_ok() {
            return true;
        }

        if attempt + 1 < attempts {
            retry_delay = (retry_delay * 2).min(max_delay);
        }
    }

    false
}
