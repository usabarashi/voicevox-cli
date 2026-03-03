use tokio::runtime::Handle;

/// Runs a potentially non-Send async task on a blocking worker thread using the current runtime.
///
/// This isolates the `spawn_blocking + block_on` bridge required by audio playback code.
pub fn spawn_non_send_tool_task<F>(future_factory: F)
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + Send + 'static,
{
    let runtime_handle = Handle::current();
    tokio::task::spawn_blocking(move || {
        runtime_handle.block_on(future_factory());
    });
}
