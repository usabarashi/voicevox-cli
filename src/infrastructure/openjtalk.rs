use anyhow::{anyhow, Result};
use voicevox_core::blocking::OpenJtalk;

use crate::infrastructure::paths::find_openjtalk_dict;

/// Initializes OpenJTalk from installed dictionary resources.
///
/// # Errors
///
/// Returns an error when dictionary path resolution or OpenJTalk creation fails.
pub fn initialize() -> Result<OpenJtalk> {
    let dict_path = find_openjtalk_dict()?;
    let dict_path = dict_path
        .to_str()
        .ok_or_else(|| anyhow!("Invalid OpenJTalk dictionary path"))?;

    OpenJtalk::new(dict_path).map_err(|e| anyhow!("Failed to initialize OpenJTalk: {e}"))
}
