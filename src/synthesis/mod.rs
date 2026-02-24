pub mod executor;
pub mod service;
pub mod streaming;

pub use executor::{prepare_backend, prepare_backend_with_config, PreparedBackend};
pub use service::{
    synthesize_bytes, synthesize_streaming_segments, synthesize_streaming_to_sink,
    validate_basic_request, TextSynthesisRequest,
};
pub use streaming::{StreamingSynthesizer, TextSegmenter, TextSplitter};
