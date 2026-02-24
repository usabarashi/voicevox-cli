use anyhow::{anyhow, Result};
use rodio::Sink;

use crate::client::DaemonClient;
use crate::ipc::{
    is_valid_synthesis_rate, OwnedSynthesizeOptions, MAX_SYNTHESIS_RATE, MIN_SYNTHESIS_RATE,
};

use super::StreamingSynthesizer;

pub struct TextSynthesisRequest<'a> {
    pub text: &'a str,
    pub style_id: u32,
    pub rate: f32,
}

pub fn validate_basic_request(request: &TextSynthesisRequest<'_>) -> Result<()> {
    if request.text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }

    if !is_valid_synthesis_rate(request.rate) {
        return Err(anyhow!(
            "Rate must be between {MIN_SYNTHESIS_RATE:.1} and {MAX_SYNTHESIS_RATE:.1}, got: {}",
            request.rate
        ));
    }

    Ok(())
}

pub async fn synthesize_bytes(
    client: &mut DaemonClient,
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
