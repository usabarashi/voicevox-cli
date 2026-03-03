pub use crate::domain::synthesis::limits::{
    DEFAULT_SYNTHESIS_RATE, MAX_SYNTHESIS_RATE, MAX_SYNTHESIS_TEXT_LENGTH, MIN_SYNTHESIS_RATE,
    is_valid_synthesis_rate,
};
pub const MAX_DAEMON_REQUEST_FRAME_BYTES: usize = 256 * 1024;
pub const MAX_DAEMON_RESPONSE_FRAME_BYTES: usize = 128 * 1024 * 1024;
