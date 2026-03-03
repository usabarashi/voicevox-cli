use anyhow::Result;

use crate::domain::synthesis::TextSynthesisRequest;
use crate::infrastructure::daemon::rpc::DaemonRpcClient;
use crate::interface::ipc::OwnedSynthesizeOptions;

pub struct DaemonSynthesizer {
    daemon_rpc: DaemonRpcClient,
}

impl DaemonSynthesizer {
    #[must_use]
    pub fn new_with_client(daemon_rpc: DaemonRpcClient) -> Self {
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
