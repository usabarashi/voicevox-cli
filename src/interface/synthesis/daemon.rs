use anyhow::Result;

use crate::domain::synthesis::TextSynthesisRequest;
use crate::infrastructure::daemon::client::DaemonClient;
use crate::infrastructure::ipc::OwnedSynthesizeOptions;

pub struct DaemonSynthesizer {
    daemon_rpc: DaemonClient,
}

impl DaemonSynthesizer {
    #[must_use]
    pub fn new_with_client(daemon_rpc: DaemonClient) -> Self {
        Self { daemon_rpc }
    }

    pub async fn synthesize_bytes(
        &mut self,
        request: &TextSynthesisRequest<'_>,
    ) -> Result<Vec<u8>> {
        let options = OwnedSynthesizeOptions { rate: request.rate };
        self.daemon_rpc
            .synthesize(request.text, request.style_id, options)
            .await
    }
}
