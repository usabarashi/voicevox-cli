mod cleanup;
mod install;
mod status;
mod update;

use crate::infrastructure::paths::{
    find_models_dir, find_onnxruntime, find_openjtalk_dict, get_default_voicevox_dir,
};
use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub use cleanup::{cleanup_unnecessary_files, count_vvm_files_recursive};
pub use install::{ensure_resources_available, launch_downloader_for_user};
pub use status::{check_updates, show_version_info};
pub use update::{update_dictionary_only, update_models_only, update_specific_model};

pub(crate) fn collect_missing_resources() -> Vec<&'static str> {
    [
        ("onnxruntime", find_onnxruntime().is_err()),
        ("dict", find_openjtalk_dict().is_err()),
        ("models", find_models_dir().is_err()),
    ]
    .into_iter()
    .filter_map(|(name, missing)| missing.then_some(name))
    .collect()
}

/// Returns startup-critical resources that are currently missing.
#[must_use]
pub fn missing_startup_resources() -> Vec<&'static str> {
    collect_missing_resources()
}

/// Returns true when all startup-critical resources are available.
#[must_use]
pub fn has_startup_resources() -> bool {
    collect_missing_resources().is_empty()
}

pub(crate) fn default_download_target_dir() -> PathBuf {
    std::env::var_os("HOME").map_or_else(
        || PathBuf::from("./voicevox"),
        |_| get_default_voicevox_dir(),
    )
}

/// Find the voicevox-download binary.
///
/// # Errors
///
/// Returns an error when the downloader executable cannot be found.
pub(crate) fn find_downloader_binary() -> Result<PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let downloader = current_exe.with_file_name("voicevox-download");
        if downloader.exists() {
            return Ok(downloader);
        }

        if let Some(pkg_root) = current_exe.parent().and_then(|p| p.parent()) {
            let pkg_downloader = pkg_root.join("bin/voicevox-download");
            if pkg_downloader.exists() {
                return Ok(pkg_downloader);
            }
        }
    }

    Err(anyhow!("voicevox-download not found"))
}

/// Backward-compatible alias used by CLI call paths.
///
/// # Errors
///
/// Returns an error if resource setup fails.
pub async fn ensure_models_available() -> Result<()> {
    ensure_resources_available().await
}
