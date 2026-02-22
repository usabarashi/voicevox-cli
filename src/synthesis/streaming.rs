use anyhow::{Context, Result};
use rodio::{Decoder, Sink};
use std::io::Cursor;

use crate::client::DaemonClient;
use crate::config::Config;

pub struct StreamingSynthesizer {
    daemon_client: DaemonClient,
    text_splitter: TextSplitter,
}

impl StreamingSynthesizer {
    /// Creates a streaming synthesizer backed by a daemon client and splitter config.
    ///
    /// # Errors
    ///
    /// Returns an error if the daemon cannot be reached.
    pub async fn new() -> Result<Self> {
        let daemon_client = DaemonClient::connect_with_retry().await?;
        let config = Config::default();
        let text_splitter = TextSplitter::from_config(&config.text_splitter);
        Ok(Self {
            daemon_client,
            text_splitter,
        })
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
        let segments = self.text_splitter.split(text);
        sink.play();
        let options = crate::ipc::OwnedSynthesizeOptions { rate };

        for (i, segment) in segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| !segment.trim().is_empty())
        {
            let wav_data = self
                .daemon_client
                .synthesize(segment, style_id, options)
                .await
                .with_context(|| format!("Failed to synthesize segment {i}: {segment}"))?;

            let cursor = Cursor::new(wav_data);
            let source = Decoder::new(cursor)
                .with_context(|| format!("Failed to decode audio for segment {i}"))?;

            sink.append(source);
        }

        Ok(())
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
        let delimiters = config
            .delimiters
            .iter()
            .filter_map(|s| s.chars().next())
            .collect::<Vec<_>>();
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

    #[must_use]
    pub fn split(&self, text: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_segment = String::new();
        let mut current_len = 0;
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            current_segment.push(ch);
            current_len += 1;

            if self.delimiters.contains(&ch) {
                self.consume_consecutive_delimiters(&mut chars, &mut current_segment);
                segments.push(std::mem::take(&mut current_segment));
                current_len = 0;
            } else if current_len >= self.max_length {
                current_len = self.handle_long_segment(&mut segments, &mut current_segment);
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
            if !self.delimiters.contains(&next_ch) {
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
    ) -> usize {
        if let Some(break_pos) = self.find_break_position(current_segment) {
            let rest = current_segment.split_off(break_pos);
            segments.push(std::mem::replace(current_segment, rest));
            current_segment.chars().count()
        } else {
            segments.push(std::mem::take(current_segment));
            0
        }
    }

    fn find_break_position(&self, text: &str) -> Option<usize> {
        text.char_indices()
            .enumerate()
            .take_while(|(i, _)| *i < self.max_length)
            .filter_map(|(_, (byte_idx, ch))| {
                matches!(ch, ' ' | '、' | ',').then_some(byte_idx + ch.len_utf8())
            })
            .last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
