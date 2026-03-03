use anyhow::{anyhow, Result};

use super::limits::{
    is_valid_synthesis_rate, MAX_SYNTHESIS_RATE, MAX_SYNTHESIS_TEXT_LENGTH, MIN_SYNTHESIS_RATE,
};

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

    let text_len = request.text.chars().count();
    if text_len > MAX_SYNTHESIS_TEXT_LENGTH {
        return Err(anyhow!(
            "Text too long: {text_len} characters (max: {MAX_SYNTHESIS_TEXT_LENGTH})"
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
