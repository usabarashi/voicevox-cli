use crate::paths::{
    find_models_dir, find_onnxruntime, find_openjtalk_dict, get_default_voicevox_dir,
};
use crate::voice::scan_available_models;
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub use super::download_cleanup::{cleanup_unnecessary_files, count_vvm_files_recursive};
pub use super::download_install::{ensure_resources_available, launch_downloader_for_user};
pub use super::download_update::{
    update_dictionary_only, update_models_only, update_specific_model,
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

pub(crate) fn default_download_target_dir() -> PathBuf {
    std::env::var_os("HOME").map_or_else(
        || PathBuf::from("./voicevox"),
        |_| get_default_voicevox_dir(),
    )
}

/// Find the voicevox-download binary
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

/// Ensures VOICEVOX voice models are available, prompting for download if needed.
///
/// This function now checks all resources (ONNX Runtime, dictionary, models) and
/// downloads any missing components. It replaces the previous models-only check.
///
/// # Returns
///
/// * `Ok(())` - All resources are available or successfully downloaded
/// * `Err` - User declined download or download failed
///
/// # Note
///
/// This function requires user interaction and should not be used in
/// non-interactive environments (e.g., MCP server, automated scripts).
///
/// # Errors
///
/// Returns an error if resource detection, user input, or downloads fail.
pub async fn ensure_models_available() -> Result<()> {
    ensure_resources_available().await
}

/// Prints currently installed resources and available update commands.
///
/// # Errors
///
/// Returns an error if installed model scanning fails.
pub fn check_updates() -> Result<()> {
    println!("Checking for available updates...");
    let current_models = scan_available_models()?;

    println!("Current installation status:");
    println!("  Voice models: {} VVM files", current_models.len());
    for model in &current_models {
        println!(
            "    Model {} ({})",
            model.model_id,
            model.file_path.display()
        );
    }

    match find_openjtalk_dict() {
        Ok(dict_path) => {
            println!("  Dictionary: {}", dict_path.display());
        }
        Err(_) => {
            println!("  Dictionary: Not found");
        }
    }

    println!();
    println!("Update options:");
    println!("  --update-models     Update all voice models");
    println!("  --update-dict       Update dictionary only");
    println!("  --update-model N    Update specific model N");

    Ok(())
}

/// Prints version and installed resource information for diagnostics.
///
/// # Errors
///
/// Returns an error if installed model scanning or file metadata queries fail.
pub fn show_version_info() -> Result<()> {
    println!("VOICEVOX CLI Version Information");
    println!("=====================================");

    println!("Application: v{}", env!("CARGO_PKG_VERSION"));
    let current_models = scan_available_models()?;

    println!("Voice Models: {} installed", current_models.len());
    for model in &current_models {
        let modified = get_file_modified(&model.file_path)?;
        println!(
            "  Model {}: {} ({})",
            model.model_id,
            model
                .file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            modified
        );
    }

    match find_openjtalk_dict() {
        Ok(dict_path) => {
            println!("Dictionary: {}", dict_path.display());
        }
        Err(_) => {
            println!("Dictionary: Not installed");
        }
    }

    Ok(())
}

fn get_file_modified(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(format!("{modified:?}"))
}
