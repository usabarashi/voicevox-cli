#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod interface;
pub mod ipc;

pub use infrastructure::core::{CoreSynthesis, VoicevoxCore};
pub use infrastructure::paths::{
    find_models_dir, find_models_dir_client, find_onnxruntime, find_openjtalk_dict, get_socket_path,
};
pub use infrastructure::voicevox::{
    get_model_for_voice_id, resolve_voice_dynamic, scan_available_models, AvailableModel, Speaker,
    Style,
};
pub use ipc::{
    DaemonErrorCode, DaemonRequest, DaemonResponse, OwnedRequest, OwnedResponse,
    OwnedSynthesizeOptions, SynthesizeOptions,
};
