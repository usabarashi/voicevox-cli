use anyhow::{anyhow, Result};

use crate::domain::synthesis::limits::DEFAULT_SYNTHESIS_RATE;

pub const MAX_STYLE_ID: u32 = 1000;

#[derive(Debug, Clone)]
pub struct SynthesizeParams {
    pub text: String,
    pub style_id: u32,
    pub rate: f32,
    pub streaming: bool,
}

#[must_use]
pub const fn is_valid_style_id(id: u32) -> bool {
    id <= MAX_STYLE_ID
}

#[must_use]
pub const fn default_rate() -> f32 {
    DEFAULT_SYNTHESIS_RATE
}

#[must_use]
pub const fn default_streaming() -> bool {
    true
}

#[must_use]
pub fn text_char_count(text: &str) -> usize {
    text.chars().count()
}

pub fn validate_style_id(style_id: u32) -> Result<()> {
    is_valid_style_id(style_id)
        .then_some(())
        .ok_or_else(|| anyhow!("Invalid style_id: {} (max: {})", style_id, MAX_STYLE_ID))
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
