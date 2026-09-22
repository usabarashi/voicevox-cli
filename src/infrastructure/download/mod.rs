mod cleanup;
mod install;
mod status;
mod update;

use crate::infrastructure::paths::{
    find_onnxruntime, find_openjtalk_dict, get_default_voicevox_dir,
};
use crate::infrastructure::voicevox::has_available_models;
use anyhow::{Result, anyhow};
use std::path::PathBuf;

pub use cleanup::{cleanup_unnecessary_files, count_vvm_files_recursive};
#[doc(hidden)]
pub use install::{
    DownloadAttempt, DownloadFailure, DownloadPhase, DownloadTracker, InstallReport,
    MAX_DOWNLOAD_ATTEMPTS, ResourceInstaller, install_with_retries,
};
pub use install::{
    default_models_download_target_dir, download_missing_resources, launch_models_downloader,
    missing_resource_descriptions,
};
pub use status::{UpdateStatus, VersionInfo, collect_update_status, collect_version_info};
pub use update::{UpdateKind, UpdateOutcome, update_dictionary_only, update_models_only};

/// Maps presence flags to the list of missing startup resource names.
///
/// Kept pure so the model-presence policy can be tested without touching the
/// filesystem: a directory that merely exists is not enough, at least one `.vvm`
/// file must be discoverable (see [`has_available_models`]).
fn missing_resources(
    onnxruntime_present: bool,
    dict_present: bool,
    models_present: bool,
) -> Vec<&'static str> {
    [
        ("onnxruntime", onnxruntime_present),
        ("dict", dict_present),
        ("models", models_present),
    ]
    .into_iter()
    .filter_map(|(name, present)| (!present).then_some(name))
    .collect()
}

pub(crate) fn collect_missing_resources() -> Vec<&'static str> {
    missing_resources(
        find_onnxruntime().is_ok(),
        find_openjtalk_dict().is_ok(),
        // Must not be `find_models_dir().is_err()`: that function falls back to
        // any existing app-data directory (e.g. one created by the VOICEVOX
        // GUI), so it succeeds even when no `.vvm` files are installed. Setup
        // would then be skipped, the daemon would start with an empty catalog,
        // and every request would fail with "Unknown style/model ID".
        has_available_models(),
    )
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

#[cfg(test)]
mod tests {
    use super::missing_resources;

    #[test]
    fn reports_all_resources_when_none_present() {
        assert_eq!(
            missing_resources(false, false, false),
            vec!["onnxruntime", "dict", "models"]
        );
    }

    #[test]
    fn models_dir_without_vvm_files_is_reported_missing() {
        // Regression: an existing voicevox data directory that contains no `.vvm`
        // files must not satisfy the models requirement.
        assert_eq!(missing_resources(true, true, false), vec!["models"]);
    }

    #[test]
    fn reports_nothing_when_all_resources_present() {
        assert!(missing_resources(true, true, true).is_empty());
    }
}
