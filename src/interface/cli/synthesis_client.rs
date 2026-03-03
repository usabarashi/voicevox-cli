use anyhow::Result;
use rodio::Sink;

use crate::domain::synthesis::TextSynthesisRequest;
use crate::interface::ipc::OwnedSynthesizeOptions;

use super::daemon_rpc::DaemonRpcClient;
use super::streaming_synthesizer::StreamingSynthesizer;

pub async fn synthesize_bytes(
    client: &mut DaemonRpcClient,
    request: &TextSynthesisRequest<'_>,
) -> Result<Vec<u8>> {
    let options = OwnedSynthesizeOptions { rate: request.rate };
    client
        .synthesize(request.text, request.style_id, options)
        .await
}

pub async fn synthesize_streaming_to_sink(
    synthesizer: &mut StreamingSynthesizer,
    request: &TextSynthesisRequest<'_>,
    sink: &Sink,
) -> Result<()> {
    synthesizer
        .synthesize_streaming(request.text, request.style_id, request.rate, sink)
        .await
}

pub async fn synthesize_streaming_segments(
    synthesizer: &mut StreamingSynthesizer,
    request: &TextSynthesisRequest<'_>,
) -> Result<Vec<Vec<u8>>> {
    synthesizer
        .synthesize_streaming_segments(request.text, request.style_id, request.rate)
        .await
}
