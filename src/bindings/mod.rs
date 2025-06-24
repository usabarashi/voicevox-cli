pub mod manual;

#[cfg(feature = "dynamic_voicevox")]
pub mod dynamic;

// Re-export based on feature flags
#[cfg(feature = "use_bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(feature = "use_bindgen")]
pub const VOICEVOX_RESULT_OK: i32 = 0;

#[cfg(not(feature = "use_bindgen"))]
pub use manual::*;

#[cfg(feature = "dynamic_voicevox")]
pub use dynamic::DynamicVoicevoxCore;