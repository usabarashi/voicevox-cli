use anyhow::{Context, Result};
use rodio::{Decoder, Sink};
use std::io::Cursor;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::client::DaemonClient;
use crate::config::Config;

/// Type alias for progress notification sender
/// Tuple: (current_progress, total_progress, status_message)
type ProgressSender = mpsc::Sender<(f64, Option<f64>, Option<String>)>;

/// Check if a string contains only punctuation and symbols (no synthesizable text)
fn is_punctuation_only(s: &str) -> bool {
    s.chars().all(|c| {
        // ASCII punctuation
        c.is_ascii_punctuation()
            // Whitespace (including full-width)
            || c.is_whitespace()
            || c == '　' // Full-width space
            // Basic Japanese punctuation
            || matches!(
                c,
                '。' | '、' | '！' | '？' | '．' | '，'
            )
            // Japanese quotation marks
            || matches!(
                c,
                '「' | '」' | '『' | '』' | '〝' | '〟'
            )
            // Parentheses (Japanese variants)
            || matches!(
                c,
                '（' | '）' | '［' | '］' | '｛' | '｝' | '【' | '】'
                    | '〔' | '〕' | '〈' | '〉' | '《' | '》' | '〖' | '〗' | '〘' | '〙'
            )
            // Dashes and lines
            || matches!(
                c,
                '―' | '－' | '─' | '━' | '—' | '–' | '〜' | '～' | '‐'
            )
            // Dots and ellipsis
            || matches!(
                c,
                '…' | '‥' | '・' | '•' | '◦' | '‣' | '⁃'
            )
            // Special marks
            || matches!(
                c,
                '※' | '〃' | '々' | '゜' | '゛' | '†' | '‡' | '§' | '¶'
            )
            // Mathematical symbols (commonly used in text)
            || matches!(
                c,
                '×' | '÷' | '±' | '°' | '′' | '″' | '‰' | '‱'
            )
            // Arrows
            || matches!(
                c,
                '→' | '←' | '↑' | '↓' | '⇒' | '⇔' | '⇄' | '⇅'
            )
            // Zero-width characters
            || matches!(
                c,
                '\u{200B}' | // Zero-width space
                '\u{200C}' | // Zero-width non-joiner
                '\u{200D}' | // Zero-width joiner
                '\u{200E}' | // Left-to-right mark
                '\u{200F}' | // Right-to-left mark
                '\u{FEFF}'   // Zero-width no-break space (BOM)
            )
    })
}

pub struct StreamingSynthesizer {
    daemon_client: DaemonClient,
    text_splitter: TextSplitter,
}

impl StreamingSynthesizer {
    pub async fn new() -> Result<Self> {
        let daemon_client = DaemonClient::connect_with_retry().await?;
        let config = Config::default();
        let text_splitter = TextSplitter::from_config(&config.text_splitter);
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
            // Skip empty segments or segments containing only punctuation/symbols
            let trimmed = segment.trim();
            if trimmed.is_empty() || is_punctuation_only(trimmed) {
                continue;
            }

            let options = crate::ipc::OwnedSynthesizeOptions { rate };
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

    /// Synthesize with cancellation and progress support
    /// Returns Ok(true) if cancelled, Ok(false) if completed normally
    pub async fn synthesize_streaming_with_cancellation(
        &mut self,
        text: &str,
        style_id: u32,
        rate: f32,
        sink: &Sink,
        cancel_token: &CancellationToken,
        progress_tx: Option<&ProgressSender>,
    ) -> Result<bool> {
        let segments = self.text_splitter.split(text);
        let total = segments.len() as f64;

        for (i, segment) in segments.iter().enumerate() {
            // Check cancellation before each segment
            if cancel_token.is_cancelled() {
                return Ok(true); // Cancelled
            }

            // Skip empty segments or segments containing only punctuation/symbols
            let trimmed = segment.trim();
            if trimmed.is_empty() || is_punctuation_only(trimmed) {
                continue;
            }

            // Report progress
            if let Some(tx) = progress_tx {
                let progress = if total > 0.0 {
                    (i as f64 / total) * 100.0
                } else {
                    0.0
                };
                let _ = tx
                    .send((
                        progress,
                        Some(100.0),
                        Some(format!("Synthesizing segment {}/{}", i + 1, total as usize)),
                    ))
                    .await;
            }

            let options = crate::ipc::OwnedSynthesizeOptions { rate };
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

        // Final progress update
        if let Some(tx) = progress_tx {
            let _ = tx
                .send((100.0, Some(100.0), Some("Reading aloud...".to_string())))
                .await;
        }

        Ok(false) // Not cancelled
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
    pub fn from_config(config: &crate::config::TextSplitterConfig) -> Self {
        // Convert string delimiters to chars
        let delimiters: Vec<char> = config
            .delimiters
            .iter()
            .filter_map(|s| s.chars().next())
            .collect();

        Self {
            delimiters,
            max_length: config.max_length,
        }
    }

    pub fn split(&self, text: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current_segment = String::new();
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            current_segment.push(ch);

            if self.delimiters.contains(&ch) {
                self.consume_consecutive_delimiters(&mut chars, &mut current_segment);
                segments.push(current_segment.clone());
                current_segment.clear();
            } else if current_segment.chars().count() >= self.max_length {
                self.handle_long_segment(&mut segments, &mut current_segment);
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

    fn handle_long_segment(&self, segments: &mut Vec<String>, current_segment: &mut String) {
        if let Some(break_pos) = self.find_break_position(current_segment) {
            let (first, rest) = current_segment.split_at(break_pos);
            segments.push(first.to_string());
            *current_segment = rest.to_string();
        } else {
            segments.push(current_segment.clone());
            current_segment.clear();
        }
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

    #[test]
    fn test_is_punctuation_only() {
        // Basic Japanese punctuation
        assert!(is_punctuation_only("」"));
        assert!(is_punctuation_only("！"));
        assert!(is_punctuation_only("？"));
        assert!(is_punctuation_only("。"));
        assert!(is_punctuation_only("、"));
        assert!(is_punctuation_only("．"));
        assert!(is_punctuation_only("，"));
        assert!(is_punctuation_only("。、"));

        // Quotation marks
        assert!(is_punctuation_only("「」"));
        assert!(is_punctuation_only("『』"));
        assert!(is_punctuation_only("〝〟"));

        // Various brackets
        assert!(is_punctuation_only("（）"));
        assert!(is_punctuation_only("［］"));
        assert!(is_punctuation_only("｛｝"));
        assert!(is_punctuation_only("【】"));
        assert!(is_punctuation_only("〔〕"));
        assert!(is_punctuation_only("〈〉"));
        assert!(is_punctuation_only("《》"));

        // ASCII punctuation
        assert!(is_punctuation_only("..."));
        assert!(is_punctuation_only("()"));
        assert!(is_punctuation_only("[]"));
        assert!(is_punctuation_only("{}"));
        assert!(is_punctuation_only("\"\""));
        assert!(is_punctuation_only("''"));

        // Dashes and lines
        assert!(is_punctuation_only("―"));
        assert!(is_punctuation_only("－"));
        assert!(is_punctuation_only("—"));
        assert!(is_punctuation_only("–"));
        assert!(is_punctuation_only("〜"));
        assert!(is_punctuation_only("～"));

        // Dots and ellipsis
        assert!(is_punctuation_only("…"));
        assert!(is_punctuation_only("‥"));
        assert!(is_punctuation_only("・"));
        assert!(is_punctuation_only("•"));

        // Special marks
        assert!(is_punctuation_only("※"));
        assert!(is_punctuation_only("〃"));
        assert!(is_punctuation_only("々"));
        assert!(is_punctuation_only("゜"));
        assert!(is_punctuation_only("゛"));

        // Mathematical symbols
        assert!(is_punctuation_only("×"));
        assert!(is_punctuation_only("÷"));
        assert!(is_punctuation_only("±"));
        assert!(is_punctuation_only("°"));

        // Arrows
        assert!(is_punctuation_only("→"));
        assert!(is_punctuation_only("←"));
        assert!(is_punctuation_only("↑"));
        assert!(is_punctuation_only("↓"));

        // Whitespace
        assert!(is_punctuation_only(" "));
        assert!(is_punctuation_only("　")); // Full-width space
        assert!(is_punctuation_only("  "));
        assert!(is_punctuation_only("\t"));
        assert!(is_punctuation_only("\n"));

        // Zero-width characters
        assert!(is_punctuation_only("\u{200B}")); // Zero-width space
        assert!(is_punctuation_only("\u{200C}")); // Zero-width non-joiner
        assert!(is_punctuation_only("\u{200D}")); // Zero-width joiner

        // Combinations
        assert!(is_punctuation_only("。、！？"));
        assert!(is_punctuation_only("「」『』"));
        assert!(is_punctuation_only("...　"));
        assert!(is_punctuation_only("※→"));

        // Should be false for strings with actual text
        assert!(!is_punctuation_only("こんにちは"));
        assert!(!is_punctuation_only("こんにちは！"));
        assert!(!is_punctuation_only("「こんにちは」"));
        assert!(!is_punctuation_only("a"));
        assert!(!is_punctuation_only("A"));
        assert!(!is_punctuation_only("1"));
        assert!(!is_punctuation_only("あ"));
        assert!(!is_punctuation_only("ア"));
        assert!(!is_punctuation_only("亜"));
        assert!(!is_punctuation_only("！あ"));
        assert!(!is_punctuation_only("あ！"));

        // Edge cases with mixed content
        assert!(!is_punctuation_only("a."));
        assert!(!is_punctuation_only(".a"));
        assert!(!is_punctuation_only("1+1"));
        assert!(!is_punctuation_only("100%"));
    }

    #[test]
    fn test_text_splitter_jugemu_patterns() {
        // Regression test for Jugemu story patterns that previously failed
        // These texts caused failures because TextSplitter creates punctuation-only segments
        // like "」" which VOICEVOX Core cannot parse
        let splitter = TextSplitter::default();

        // Pattern 1: "「それも良い！他には？」「海砂利水魚..."
        // This splits into segments including "」" which is punctuation-only
        let text1 = "「それも良い！他には？」「海砂利水魚、水行末、雲来末、風来末なども縁起が良い」「どれも素晴らしい！」";
        let segments1 = splitter.split(text1);

        // Count how many segments are punctuation-only
        let punctuation_only_count1 = segments1
            .iter()
            .filter(|s| is_punctuation_only(s.trim()))
            .count();

        // Verify that punctuation-only segments are detected
        assert!(
            punctuation_only_count1 > 0,
            "Expected to find punctuation-only segments (like '」'), found none"
        );

        // Verify that there are also valid segments
        let valid_segments1: Vec<_> = segments1
            .iter()
            .filter(|s| !s.trim().is_empty() && !is_punctuation_only(s.trim()))
            .collect();
        assert!(
            !valid_segments1.is_empty(),
            "Expected to find valid text segments"
        );

        // Pattern 2: Similar test for another pattern
        let text2 = "親は考えたのだ。「どれも良い名前だなあ。よし、全部つけてしまおう！」";
        let segments2 = splitter.split(text2);

        let valid_segments2: Vec<_> = segments2
            .iter()
            .filter(|s| !s.trim().is_empty() && !is_punctuation_only(s.trim()))
            .collect();
        assert!(!valid_segments2.is_empty());

        // Pattern 3: Edge case with nested quotations
        let text3 =
            "「グーリンダイのポンポコピーのポンポコナーの、長久命の長助が、池に落ちたー！」";
        let segments3 = splitter.split(text3);

        let valid_segments3: Vec<_> = segments3
            .iter()
            .filter(|s| !s.trim().is_empty() && !is_punctuation_only(s.trim()))
            .collect();
        assert!(!valid_segments3.is_empty());

        // Critical assertion: Ensure is_punctuation_only correctly identifies problematic segments
        assert!(
            is_punctuation_only("」"),
            "Failed to detect closing bracket as punctuation-only"
        );
        assert!(
            is_punctuation_only("！"),
            "Failed to detect exclamation mark as punctuation-only"
        );
        assert!(
            is_punctuation_only("？"),
            "Failed to detect question mark as punctuation-only"
        );
    }
}
