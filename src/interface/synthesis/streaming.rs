use anyhow::{Context, Result};
use rodio::{Decoder, Sink};
use std::io::Cursor;

use crate::config::Config;
use crate::domain::synthesis::{TextSegmenter, TextSplitter};
use crate::infrastructure::daemon::client::DaemonClient;

pub struct StreamingSynthesizer {
    daemon_rpc: DaemonClient,
    text_segmenter: Box<dyn TextSegmenter + Send + Sync>,
}

impl StreamingSynthesizer {
    /// Creates a streaming synthesizer backed by a daemon client and splitter config.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon cannot be reached.
    pub async fn new() -> Result<Self> {
        let daemon_rpc = DaemonClient::connect_with_retry().await?;
        Self::new_with_client_and_config(daemon_rpc, &Config::default())
    }

    /// Creates a streaming synthesizer with an already-connected daemon client.
    #[allow(clippy::missing_errors_doc)]
    pub fn new_with_client(daemon_rpc: DaemonClient) -> Result<Self> {
        Self::new_with_client_and_config(daemon_rpc, &Config::default())
    }

    /// Creates a streaming synthesizer with explicit configuration injection.
    #[allow(clippy::missing_errors_doc)]
    pub fn new_with_client_and_config(
        daemon_rpc: DaemonClient,
        config: &Config,
    ) -> Result<Self> {
        let delimiters = config
            .text_splitter
            .delimiters
            .iter()
            .filter_map(|s| s.chars().next())
            .collect::<Vec<_>>();
        let text_segmenter = Box::new(TextSplitter::new(
            delimiters,
            config.text_splitter.max_length,
        ));
        Ok(Self {
            daemon_rpc,
            text_segmenter,
        })
    }

    /// Creates a streaming synthesizer with an explicit segmentation strategy.
    #[allow(clippy::missing_errors_doc)]
    pub fn new_with_client_and_segmenter(
        daemon_rpc: DaemonClient,
        text_segmenter: Box<dyn TextSegmenter + Send + Sync>,
    ) -> Result<Self> {
        Ok(Self {
            daemon_rpc,
            text_segmenter,
        })
    }

    /// Synthesizes text in segments and returns synthesized WAV segments.
    ///
    /// # Errors
    ///
    /// Returns an error if segment synthesis fails.
    pub async fn request_streaming_synthesis_segments(
        &mut self,
        text: &str,
        style_id: u32,
        rate: f32,
    ) -> Result<Vec<Vec<u8>>> {
        let segments = self.text_segmenter.split(text);
        let options = crate::infrastructure::ipc::OwnedSynthesizeOptions { rate };
        let mut wav_segments = Vec::new();

        for (i, segment) in segments
            .iter()
            .filter(|segment| !segment.trim().is_empty())
            .enumerate()
        {
            let wav_data = self
                .daemon_rpc
                .synthesize(segment, style_id, options)
                .await
                .with_context(|| format!("Failed to synthesize segment {i}: {segment}"))?;
            wav_segments.push(wav_data);
        }

        Ok(wav_segments)
    }

    /// Appends synthesized WAV segments to the provided sink.
    ///
    /// # Errors
    ///
    /// Returns an error if any audio segment cannot be decoded.
    pub fn append_segments_to_sink(&self, wav_segments: &[Vec<u8>], sink: &Sink) -> Result<()> {
        sink.play();
        for (i, wav_data) in wav_segments.iter().enumerate() {
            let cursor = Cursor::new(wav_data.clone());
            let source = Decoder::new(cursor)
                .with_context(|| format!("Failed to decode audio for segment {i}"))?;
            sink.append(source);
        }
        Ok(())
    }

    /// Synthesizes text in segments and appends decoded audio to the provided sink.
    ///
    /// # Errors
    ///
    /// Returns an error if segment synthesis fails or any audio segment cannot be decoded.
    pub async fn synthesize_streaming(
        &mut self,
        text: &str,
        style_id: u32,
        rate: f32,
        sink: &Sink,
    ) -> Result<()> {
        let wav_segments = self
            .request_streaming_synthesis_segments(text, style_id, rate)
            .await?;
        self.append_segments_to_sink(&wav_segments, sink)
    }
}
