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
pub use install::{
    default_models_download_target_dir, download_missing_resources, launch_models_downloader,
    missing_resource_descriptions,
};
pub use status::{collect_update_status, collect_version_info, UpdateStatus, VersionInfo};
pub use update::{
    update_dictionary_only, update_models_only, UpdateKind, UpdateOutcome,
};

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

#[must_use]
pub fn missing_startup_resources() -> Vec<&'static str> {
    collect_missing_resources()
}

#[must_use]
pub fn has_startup_resources() -> bool {
    collect_missing_resources().is_empty()
}

pub(crate) fn default_download_target_dir() -> PathBuf {
    std::env::var_os(crate::config::ENV_HOME).map_or_else(
        || PathBuf::from("./voicevox"),
        |_| get_default_voicevox_dir(),
    )
}

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
