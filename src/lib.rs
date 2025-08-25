#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod client;
pub mod config;
pub mod core;
pub mod daemon;
pub mod ipc;
pub mod mcp;
pub mod paths;
pub mod setup;
pub mod synthesis;
pub mod voice;

pub use core::{CoreSynthesis, VoicevoxCore};
pub use ipc::{
    DaemonRequest, DaemonResponse, OwnedRequest, OwnedResponse, OwnedSynthesizeOptions,
    SynthesizeOptions,
};
pub use paths::{
    find_models_dir, find_models_dir_client, find_onnxruntime, find_openjtalk_dict, get_socket_path,
};
pub use voice::{
    get_model_for_voice_id, resolve_voice_dynamic, scan_available_models, AvailableModel, Speaker,
    Style,
};
