use anyhow::{Result, anyhow};
use voicevox_core::blocking::Onnxruntime;

use crate::infrastructure::paths::find_onnxruntime;

/// Initializes ONNX Runtime from installed resources or bundled defaults.
///
/// # Errors
///
/// Returns an error when runtime loading fails.
pub fn initialize() -> Result<&'static Onnxruntime> {
    find_onnxruntime()
        .map_or_else(
            |_| Onnxruntime::load_once().perform(),
            |ort_path| Onnxruntime::load_once().filename(ort_path).perform(),
        )
        .map_err(|_| {
            anyhow!(
                "Failed to initialize ONNX Runtime. Please run 'voicevox-setup' to download required resources."
            )
        })
}
