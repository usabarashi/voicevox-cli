pub const DEFAULT_SYNTHESIS_RATE: f32 = 1.0;
pub const MIN_SYNTHESIS_RATE: f32 = 0.5;
pub const MAX_SYNTHESIS_RATE: f32 = 2.0;
pub const MAX_SYNTHESIS_TEXT_LENGTH: usize = 10_000;

#[must_use]
pub const fn is_valid_synthesis_rate(rate: f32) -> bool {
    rate >= MIN_SYNTHESIS_RATE && rate <= MAX_SYNTHESIS_RATE
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn default_rate_is_valid() {
        assert!(is_valid_synthesis_rate(DEFAULT_SYNTHESIS_RATE));
    }

    #[kani::proof]
    fn boundary_rates_are_valid() {
        assert!(is_valid_synthesis_rate(MIN_SYNTHESIS_RATE));
        assert!(is_valid_synthesis_rate(MAX_SYNTHESIS_RATE));
    }

    #[kani::proof]
    fn out_of_range_rates_are_invalid() {
        let deci_rate: i16 = kani::any();
        kani::assume(deci_rate >= -100);
        kani::assume(deci_rate <= 300);
        let rate = f32::from(deci_rate) / 10.0;

        if rate < MIN_SYNTHESIS_RATE || rate > MAX_SYNTHESIS_RATE {
            assert!(!is_valid_synthesis_rate(rate));
        }
    }
}
