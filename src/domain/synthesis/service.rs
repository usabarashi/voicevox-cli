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

#[cfg(kani)]
mod kani_proofs {
    use super::*;
    use crate::domain::synthesis::limits::{MAX_SYNTHESIS_RATE, MIN_SYNTHESIS_RATE};

    #[kani::proof]
    fn rate_validation_matches_request_result_for_valid_text() {
        let request = TextSynthesisRequest {
            text: "hello",
            style_id: kani::any(),
            rate: kani::any(),
        };

        let result = validate_basic_request(&request);

        if request.rate >= MIN_SYNTHESIS_RATE && request.rate <= MAX_SYNTHESIS_RATE {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }

    #[kani::proof]
    fn blank_text_is_rejected_regardless_of_rate() {
        let request = TextSynthesisRequest {
            text: " \n\t ",
            style_id: kani::any(),
            rate: kani::any(),
        };

        assert!(validate_basic_request(&request).is_err());
    }
}
