use anyhow::Result;
use rodio::Sink;

use crate::domain::synthesis::TextSynthesisRequest;
use crate::interface::cli::daemon_rpc::DaemonRpcClient;
use crate::interface::ipc::OwnedSynthesizeOptions;

use super::streaming_synthesis::StreamingSynthesizer;

pub async fn request_daemon_synthesis_bytes(
    client: &mut DaemonRpcClient,
    request: &TextSynthesisRequest<'_>,
) -> Result<Vec<u8>> {
    let options = OwnedSynthesizeOptions { rate: request.rate };
    client
        .synthesize(request.text, request.style_id, options)
        .await
}

pub async fn stream_synthesis_to_sink(
    synthesizer: &mut StreamingSynthesizer,
    request: &TextSynthesisRequest<'_>,
    sink: &Sink,
) -> Result<()> {
    synthesizer
        .synthesize_streaming(request.text, request.style_id, request.rate, sink)
        .await
}

pub async fn request_streaming_synthesis_segments(
    synthesizer: &mut StreamingSynthesizer,
    request: &TextSynthesisRequest<'_>,
) -> Result<Vec<Vec<u8>>> {
    synthesizer
        .request_streaming_synthesis_segments(request.text, request.style_id, request.rate)
        .await
}
