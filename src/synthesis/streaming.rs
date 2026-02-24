use anyhow::{Context, Result};
use rodio::{Decoder, Sink};
use std::io::Cursor;

use crate::client::DaemonClient;
use crate::config::Config;

pub struct StreamingSynthesizer {
    daemon_client: DaemonClient,
    text_segmenter: Box<dyn TextSegmenter + Send + Sync>,
}

pub trait TextSegmenter {
    fn split(&self, text: &str) -> Vec<String>;
}

impl StreamingSynthesizer {
    /// Creates a streaming synthesizer backed by a daemon client and splitter config.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon cannot be reached.
    pub async fn new() -> Result<Self> {
        let daemon_client = DaemonClient::connect_with_retry().await?;
        Self::new_with_client_and_config(daemon_client, &Config::default())
    }

    /// Creates a streaming synthesizer with an already-connected daemon client.
    #[allow(clippy::missing_errors_doc)]
    pub fn new_with_client(daemon_client: DaemonClient) -> Result<Self> {
        Self::new_with_client_and_config(daemon_client, &Config::default())
    }

    /// Creates a streaming synthesizer with explicit configuration injection.
    #[allow(clippy::missing_errors_doc)]
    pub fn new_with_client_and_config(
        daemon_client: DaemonClient,
        config: &Config,
    ) -> Result<Self> {
        let text_segmenter = Box::new(TextSplitter::from_config(&config.text_splitter));
        Ok(Self {
            daemon_client,
            text_segmenter,
        })
    }

    /// Creates a streaming synthesizer with an explicit segmentation strategy.
    #[allow(clippy::missing_errors_doc)]
    pub fn new_with_client_and_segmenter(
        daemon_client: DaemonClient,
        text_segmenter: Box<dyn TextSegmenter + Send + Sync>,
    ) -> Result<Self> {
        Ok(Self {
            daemon_client,
            text_segmenter,
        })
    }

    /// Synthesizes text in segments and returns synthesized WAV segments.
    ///
    /// # Errors
    ///
    /// Returns an error if segment synthesis fails.
    pub async fn synthesize_streaming_segments(
        &mut self,
        text: &str,
        style_id: u32,
        rate: f32,
    ) -> Result<Vec<Vec<u8>>> {
        let segments = self.text_segmenter.split(text);
        let options = crate::ipc::OwnedSynthesizeOptions { rate };
        let mut wav_segments = Vec::new();

        for (i, segment) in segments
            .iter()
            .filter(|segment| !segment.trim().is_empty())
            .enumerate()
        {
            let wav_data = self
                .daemon_client
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
            .synthesize_streaming_segments(text, style_id, rate)
            .await?;
        self.append_segments_to_sink(&wav_segments, sink)
    }
}

#[derive(Debug, Clone)]
pub struct TextSplitter {
    delimiters: Vec<char>,
    max_length: usize,
}

impl Default for TextSplitter {
    fn default() -> Self {
        Self {
            delimiters: vec!['。', '！', '？', '．', '\n'],
            max_length: 100,
        }
    }
}

impl TextSplitter {
    #[must_use]
    pub fn from_config(config: &crate::config::TextSplitterConfig) -> Self {
        let mut delimiters = config
            .delimiters
            .iter()
            .filter_map(|s| s.chars().next())
            .collect::<Vec<_>>();
        delimiters.sort_unstable();
        delimiters.dedup();
        let max_length = config.max_length.max(1);

        if delimiters.is_empty() {
            Self {
                max_length,
                ..Self::default()
            }
        } else {
            Self {
                delimiters,
                max_length,
            }
        }
    }

    fn is_delimiter(&self, ch: char) -> bool {
        self.delimiters.contains(&ch)
    }

    #[must_use]
    pub fn split(&self, text: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_segment = String::new();
        let mut current_len = 0;
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            current_segment.push(ch);
            current_len += 1;

            if self.is_delimiter(ch) {
                self.consume_consecutive_delimiters(&mut chars, &mut current_segment);
                segments.push(std::mem::take(&mut current_segment));
                current_len = 0;
            } else if current_len >= self.max_length {
                current_len =
                    self.handle_long_segment(&mut segments, &mut current_segment, current_len);
            }
        }

        if !current_segment.trim().is_empty() {
            segments.push(current_segment);
        }

        segments
    }

    fn consume_consecutive_delimiters(
        &self,
        chars: &mut std::iter::Peekable<std::str::Chars>,
        current_segment: &mut String,
    ) {
        while let Some(&next_ch) = chars.peek() {
            if !self.is_delimiter(next_ch) {
                break;
            }
            if let Some(next_ch) = chars.next() {
                current_segment.push(next_ch);
            }
        }
    }

    fn handle_long_segment(
        &self,
        segments: &mut Vec<String>,
        current_segment: &mut String,
        current_len: usize,
    ) -> usize {
        if let Some((break_pos, head_len)) = self.find_break_position(current_segment) {
            let rest = current_segment.split_off(break_pos);
            segments.push(std::mem::replace(current_segment, rest));
            current_len.saturating_sub(head_len)
        } else {
            segments.push(std::mem::take(current_segment));
            0
        }
    }

    fn find_break_position(&self, text: &str) -> Option<(usize, usize)> {
        text.char_indices()
            .enumerate()
            .take_while(|(i, _)| *i < self.max_length)
            .filter_map(|(char_idx, (byte_idx, ch))| {
                matches!(ch, ' ' | '、' | ',').then_some((byte_idx + ch.len_utf8(), char_idx + 1))
            })
            .last()
    }
}

impl TextSegmenter for TextSplitter {
    fn split(&self, text: &str) -> Vec<String> {
        Self::split(self, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSegmenter;

    impl TextSegmenter for FixedSegmenter {
        fn split(&self, _text: &str) -> Vec<String> {
            vec!["a".to_string(), "b".to_string()]
        }
    }

    #[test]
    fn test_text_splitter_basic() {
        let splitter = TextSplitter::default();

        let text = "こんにちは。今日はいい天気ですね！明日も晴れるでしょうか？";
        let segments = splitter.split(text);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], "こんにちは。");
        assert_eq!(segments[1], "今日はいい天気ですね！");
        assert_eq!(segments[2], "明日も晴れるでしょうか？");
    }

    #[test]
    fn test_text_splitter_long_text() {
        let splitter = TextSplitter {
            delimiters: vec!['。'],
            max_length: 10,
        };

        let text = "あいうえおかきくけこさしすせそ";
        let segments = splitter.split(text);

        assert!(!segments.is_empty());
        assert!(segments[0].chars().count() <= 10);
    }

    #[test]
    fn test_text_splitter_consecutive_punctuation() {
        let splitter = TextSplitter::default();

        let text = "すごい！！！本当に？？";
        let segments = splitter.split(text);

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], "すごい！！！");
        assert_eq!(segments[1], "本当に？？");
    }

    #[test]
    fn trait_object_segmenter_is_swappable() {
        let segmenter: Box<dyn TextSegmenter + Send + Sync> = Box::new(FixedSegmenter);
        let segments = segmenter.split("ignored");
        assert_eq!(segments, vec!["a", "b"]);
    }
}
