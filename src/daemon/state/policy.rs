use tokio::sync::Mutex;

use crate::ipc::OwnedResponse;

use super::catalog::ModelCatalog;
use super::executor::DaemonSynthesisExecutor;

/// Explicitly serialized synthesis policy.
///
/// VOICEVOX core/model loading is executed under a single async mutex to keep memory usage
/// predictable under the current no-model-cache design.
pub(super) struct SerializedSynthesisPolicy {
    executor: Mutex<DaemonSynthesisExecutor>,
}

impl SerializedSynthesisPolicy {
    pub(super) fn new(executor: DaemonSynthesisExecutor) -> Self {
        Self {
            executor: Mutex::new(executor),
        }
    }

    pub(super) async fn synthesize(
        &self,
        catalog: &ModelCatalog,
        text: String,
        requested_id: u32,
        rate: f32,
    ) -> OwnedResponse {
        let executor = self.executor.lock().await;
        executor.synthesize(catalog, text, requested_id, rate)
    }
}
