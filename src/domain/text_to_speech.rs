use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::domain::synthesis::limits::{
    is_valid_synthesis_rate, DEFAULT_SYNTHESIS_RATE, MAX_SYNTHESIS_RATE, MAX_SYNTHESIS_TEXT_LENGTH,
    MIN_SYNTHESIS_RATE,
};

pub const MAX_STYLE_ID: u32 = 1000;
#[cfg(test)]
pub const MAX_TEXT_LENGTH: usize = MAX_SYNTHESIS_TEXT_LENGTH;

#[must_use]
pub const fn is_valid_style_id(id: u32) -> bool {
    id <= MAX_STYLE_ID
}

#[derive(Debug, Deserialize)]
pub struct SynthesizeParams {
    pub text: String,
    pub style_id: u32,
    #[serde(default = "default_rate")]
    pub rate: f32,
    #[serde(default = "default_streaming")]
    pub streaming: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTtsPhase {
    Attempt,
    Backoff,
    Finish,
}

const fn default_rate() -> f32 {
    DEFAULT_SYNTHESIS_RATE
}

const fn default_streaming() -> bool {
    true
}

#[must_use]
pub fn text_char_count(text: &str) -> usize {
    text.chars().count()
}

pub fn validate_synthesize_params(params: &SynthesizeParams) -> Result<()> {
    let text = params.text.trim();
    let text_char_count = text_char_count(text);
    (!text.is_empty())
        .then_some(())
        .ok_or_else(|| anyhow!("Text cannot be empty"))?;

    (text_char_count <= MAX_SYNTHESIS_TEXT_LENGTH)
        .then_some(())
        .ok_or_else(|| {
            anyhow!(
                "Text too long: {text_char_count} characters (max: {MAX_SYNTHESIS_TEXT_LENGTH})"
            )
        })?;

    is_valid_synthesis_rate(params.rate)
        .then_some(())
        .ok_or_else(|| {
            anyhow!("Rate must be between {MIN_SYNTHESIS_RATE:.1} and {MAX_SYNTHESIS_RATE:.1}")
        })?;

    is_valid_style_id(params.style_id)
        .then_some(())
        .ok_or_else(|| {
            anyhow!(
                "Invalid style_id: {} (max: {})",
                params.style_id,
                MAX_STYLE_ID
            )
        })?;

    Ok(())
}

pub fn parse_synthesize_params(arguments: Value) -> Result<SynthesizeParams> {
    let params: SynthesizeParams =
        serde_json::from_value(arguments).context("Invalid parameters for text_to_speech")?;
    validate_synthesize_params(&params)?;
    Ok(params)
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn style_id_boundary() {
        let id: u32 = kani::any();
        if id <= MAX_STYLE_ID {
            assert!(is_valid_style_id(id));
        } else {
            assert!(!is_valid_style_id(id));
        }
    }
}
