mod limits;
mod protocol;

pub use limits::{
    is_valid_synthesis_rate, DEFAULT_SYNTHESIS_RATE, MAX_DAEMON_REQUEST_FRAME_BYTES,
    MAX_DAEMON_RESPONSE_FRAME_BYTES, MAX_SYNTHESIS_RATE, MAX_SYNTHESIS_TEXT_LENGTH,
    MIN_SYNTHESIS_RATE,
};
pub use protocol::{
    DaemonErrorCode, DaemonRequest, DaemonResponse, IpcModel, IpcSpeaker, IpcStyle, OwnedRequest,
    OwnedResponse, OwnedSynthesizeOptions, SynthesizeOptions,
};
