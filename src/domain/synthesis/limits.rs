pub const DEFAULT_SYNTHESIS_RATE: f32 = 1.0;
pub const MIN_SYNTHESIS_RATE: f32 = 0.5;
pub const MAX_SYNTHESIS_RATE: f32 = 2.0;
pub const MAX_SYNTHESIS_TEXT_LENGTH: usize = 10_000;

#[must_use]
pub const fn is_valid_synthesis_rate(rate: f32) -> bool {
    rate >= MIN_SYNTHESIS_RATE && rate <= MAX_SYNTHESIS_RATE
}
