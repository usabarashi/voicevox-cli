use anyhow::{Context, Result};
use rodio::{Decoder, Sink};
use std::io::Cursor;

use crate::client::DaemonClient;

pub struct StreamingSynthesizer {
    daemon_client: DaemonClient,
    text_splitter: TextSplitter,
}

impl StreamingSynthesizer {
    pub async fn new() -> Result<Self> {
        let daemon_client = DaemonClient::connect_with_retry().await?;
        let text_splitter = TextSplitter::default();
        Ok(Self {
            daemon_client,
            text_splitter,
        })
    }

    pub async fn synthesize_streaming(
        &mut self,
        text: &str,
        style_id: u32,
        rate: f32,
        sink: &Sink,
    ) -> Result<()> {
        let segments = self.text_splitter.split(text);

        for (i, segment) in segments.iter().enumerate() {
            if segment.trim().is_empty() {
                continue;
            }

            let options = crate::ipc::OwnedSynthesizeOptions {
                rate,
                ..Default::default()
            };
            let wav_data = self
                .daemon_client
                .synthesize(segment, style_id, options)
                .await
                .with_context(|| format!("Failed to synthesize segment {i}: {segment}"))?;

            let cursor = Cursor::new(wav_data);
            let source = Decoder::new(cursor)
                .with_context(|| format!("Failed to decode audio for segment {i}"))?;

            sink.append(source);

            if i == 0 {
                sink.play();
            }
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
    pub fn split(&self, text: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_segment = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            current_segment.push(ch);

            if self.delimiters.contains(&ch) {
                while let Some(&next_ch) = chars.peek() {
                    if self.delimiters.contains(&next_ch) {
                        if let Some(next_ch) = chars.next() {
                            current_segment.push(next_ch);
                        }
                    } else {
                        break;
                    }
                }

                segments.push(current_segment.clone());
                current_segment.clear();
            } else if current_segment.chars().count() >= self.max_length {
                if let Some(break_pos) = self.find_break_position(&current_segment) {
                    let (first, rest) = current_segment.split_at(break_pos);
                    segments.push(first.to_string());
                    current_segment = rest.to_string();
                } else {
                    segments.push(current_segment.clone());
                    current_segment.clear();
                }
            }
        }

        if !current_segment.trim().is_empty() {
            segments.push(current_segment);
        }

        segments
    }

    fn find_break_position(&self, text: &str) -> Option<usize> {
        let chars: Vec<char> = text.chars().collect();
        let search_end = chars.len().min(self.max_length);
        for i in (0..search_end).rev() {
            if chars[i] == ' ' || chars[i] == '、' || chars[i] == ',' {
                return Some(text.char_indices().nth(i + 1)?.0);
            }
        }
        None
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
