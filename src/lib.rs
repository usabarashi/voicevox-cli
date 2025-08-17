//! VOICEVOX CLI - Japanese text-to-speech using VOICEVOX Core

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod core;

pub mod ipc;

pub mod voice;

pub mod paths;

pub mod setup;

pub mod client;

pub mod daemon;

pub use core::{CoreSynthesis, VoicevoxCore};
pub use ipc::{
    DaemonRequest, DaemonResponse, OwnedRequest, OwnedResponse, OwnedSynthesizeOptions,
    SynthesizeOptions,
};
pub use paths::{find_models_dir, find_models_dir_client, find_openjtalk_dict, get_socket_path};
pub use setup::{attempt_first_run_setup, is_valid_models_directory};
pub use voice::{
    get_model_for_voice_id, resolve_voice_dynamic, scan_available_models, AvailableModel, Speaker,
    Style,
};
