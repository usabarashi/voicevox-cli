use tokio::sync::Mutex;

use crate::infrastructure::core::ModelRuntime;

use super::catalog::ModelCatalog;
use super::executor::DaemonSynthesisExecutor;
use super::result::{DaemonServiceError, DaemonServiceResult};

/// Explicitly serialized synthesis policy.
///
/// VOICEVOX core/model loading is executed under a single async mutex to keep memory usage
/// predictable under the current no-model-cache design.
#[doc(hidden)]
pub struct SerializedSynthesisPolicy<R: ModelRuntime> {
    executor: Mutex<DaemonSynthesisExecutor<R>>,
}

impl<R: ModelRuntime> SerializedSynthesisPolicy<R> {
    #[must_use]
    pub fn new(executor: DaemonSynthesisExecutor<R>) -> Self {
        Self {
            executor: Mutex::new(executor),
        }
    }

    pub async fn synthesize(
        &self,
        catalog: &ModelCatalog,
        text: String,
        requested_id: u32,
        rate: f32,
    ) -> Result<DaemonServiceResult, DaemonServiceError> {
        let mut executor = self.executor.lock().await;
        executor.synthesize(catalog, text, requested_id, rate)
    }
}
