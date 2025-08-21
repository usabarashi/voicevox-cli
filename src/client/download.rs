use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use crate::paths::{
    find_models_dir, find_onnxruntime, find_openjtalk_dict, get_default_voicevox_dir,
};

/// Check and ensure all required resources are available
pub async fn ensure_resources_available() -> Result<()> {
    let mut missing_resources = Vec::new();

    if find_onnxruntime().is_err() {
        missing_resources.push("onnxruntime");
    }
    if find_openjtalk_dict().is_err() {
        missing_resources.push("dict");
    }
    if find_models_dir().is_err() {
        missing_resources.push("models");
    }
    if missing_resources.is_empty() {
        return Ok(());
    }

    println!("🎭 VOICEVOX CLI - Initial Setup Required");
    println!("The following resources need to be downloaded:");
    if missing_resources.contains(&"onnxruntime") {
        println!("  • ONNX Runtime - Neural network inference engine");
    }
    if missing_resources.contains(&"dict") {
        println!("  • OpenJTalk Dictionary - Japanese text processing");
    }
    if missing_resources.contains(&"models") {
        println!("  • Voice Models - Character voices");
    }
    println!();

    print!("Would you like to download these resources now? [Y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let response = input.trim().to_lowercase();
    if response.is_empty() || response == "y" || response == "yes" {
        println!("🔄 Starting resource download...");
        let target_dir = get_default_voicevox_dir();
        std::fs::create_dir_all(&target_dir)?;
        let downloader_path = find_downloader_binary()?;
        let only_arg = missing_resources.join(",");
        println!("📦 Downloading to: {}", target_dir.display());

        let max_retries = 3;
        let mut last_error = None;

        for attempt in 1..=max_retries {
            if attempt > 1 {
                println!("🔄 Retrying download... (Attempt {}/{})", attempt, max_retries);
                cleanup_incomplete_downloads(&target_dir);
            }

            let status = std::process::Command::new(&downloader_path)
                .arg("--only")
                .arg(&only_arg)
                .arg("--output")
                .arg(&target_dir)
                .status();

            match status {
                Ok(exit_status) if exit_status.success() => {
                    println!("✅ All resources downloaded successfully!");
                    if missing_resources.contains(&"onnxruntime") {
                        if let Ok(ort_path) = find_onnxruntime() {
                            std::env::set_var("ORT_DYLIB_PATH", ort_path);
                        }
                    }
                    return Ok(());
                }
                Ok(exit_status) => {
                    let error_msg = format!("Download failed with exit code: {:?}", exit_status.code());
                    last_error = Some(error_msg);
                }
                Err(e) => {
                    let error_msg = format!("Failed to execute downloader: {}", e);
                    last_error = Some(error_msg);
                }
            }

            if attempt < max_retries {
                println!("⏳ Download failed, waiting 2 seconds before retry...");
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }

        cleanup_incomplete_downloads(&target_dir);
        if let Some(error) = last_error {
            eprintln!("❌ Resource download failed after {} attempts: {}", max_retries, error);
        } else {
            eprintln!("❌ Resource download failed after {} attempts", max_retries);
        }
        eprintln!(
            "You can manually run: voicevox-download --only {} --output {}",
            only_arg,
            target_dir.display()
        );
        Err(anyhow!("Failed to download required resources after {} attempts", max_retries))
    } else {
        println!("Setup cancelled. You can run 'voicevox-setup' later to download resources.");
        Err(anyhow!("Required resources are not available"))
    }
}

/// Clean up incomplete downloads (temporary files, partial downloads)
fn cleanup_incomplete_downloads(target_dir: &std::path::Path) {
    if let Ok(entries) = std::fs::read_dir(target_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                let path = entry.path();

                // Remove temporary files (e.g., .tmp, .download, .partial)
                if let Some(extension) = path.extension() {
                    let ext_str = extension.to_string_lossy().to_lowercase();
                    if ext_str == "tmp" || ext_str == "download" || ext_str == "partial" {
                        if let Err(e) = std::fs::remove_file(&path) {
                            eprintln!("Warning: Failed to clean up temporary file {}: {}", path.display(), e);
                        } else {
                            println!("🧹 Cleaned up temporary file: {}", path.display());
                        }
                        continue;
                    }
                }

                // Remove very small files that might be incomplete downloads
                if file_type.is_file() {
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        // Files smaller than 1KB are likely incomplete
                        if metadata.len() < 1024 {
                            // Only remove files that look like they should be larger
                            if let Some(filename) = path.file_name() {
                                let filename_str = filename.to_string_lossy().to_lowercase();
                                if filename_str.contains("onnx") ||
                                   filename_str.contains("dict") ||
                                   filename_str.contains("model") ||
                                   filename_str.ends_with(".dylib") ||
                                   filename_str.ends_with(".so") ||
                                   filename_str.ends_with(".dll") {
                                    if let Err(e) = std::fs::remove_file(&path) {
                                        eprintln!("Warning: Failed to clean up incomplete file {}: {}", path.display(), e);
                                    } else {
                                        println!("🧹 Cleaned up incomplete file: {}", path.display());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Find the voicevox-download binary
fn find_downloader_binary() -> Result<PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut downloader = current_exe.clone();
        downloader.set_file_name("voicevox-download");
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

/// Launches VOICEVOX downloader for voice models with direct user interaction
pub async fn launch_downloader_for_user() -> Result<()> {
    let target_dir = std::env::var("HOME")
        .ok()
        .map(|_| get_default_voicevox_dir())
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    std::fs::create_dir_all(&target_dir)?;

    let downloader_path = if let Ok(current_exe) = std::env::current_exe() {
        let mut downloader = current_exe.clone();
        downloader.set_file_name("voicevox-download");
        if downloader.exists() {
            downloader
        } else if let Some(pkg_root) = current_exe.parent().and_then(|p| p.parent()) {
            let pkg_downloader = pkg_root.join("bin/voicevox-download");
            if pkg_downloader.exists() {
                pkg_downloader
            } else {
                return Err(anyhow!("voicevox-download not found"));
            }
        } else {
            return Err(anyhow!("voicevox-download not found"));
        }
    } else {
        return Err(anyhow!("Could not find voicevox-download"));
    };

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Launching VOICEVOX downloader...");
    println!("   This will download: 26+ voice models only");
    println!("   Please follow the on-screen instructions to accept license terms.");
    println!("   Press Enter when ready to continue...");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(&target_dir)
        .status()?;

    if status.success() {
        let _vvm_files = std::fs::read_dir(&target_dir)
            .map_err(|e| anyhow!("Failed to read target directory: {e}"))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().is_file()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.ends_with(".vvm"))
                    || entry.path().is_dir()
            })
            .collect::<Vec<_>>();

        let vvm_count = count_vvm_files_recursive(&target_dir);

        if vvm_count > 0 {
            println!(
                "✅ Voice models successfully downloaded to: {}",
                target_dir.display()
            );
            println!("   Found {vvm_count} VVM model files");

            cleanup_unnecessary_files(&target_dir);

            Ok(())
        } else {
            Err(anyhow!(
                "Download completed but voice model files not found in target directory"
            ))
        }
    } else {
        Err(anyhow!("Download process failed or was cancelled"))
    }
}

pub fn count_vvm_files_recursive(dir: &std::path::PathBuf) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .map(|path| match path {
                    p if p.is_file() => count_vvm_file(&p),
                    p if p.is_dir() => count_vvm_files_recursive(&p),
                    _ => 0,
                })
                .sum()
        })
        .unwrap_or(0)
}

fn count_vvm_file(path: &Path) -> usize {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.ends_with(".vvm"))
        .map(|_| 1)
        .unwrap_or(0)
}

pub fn cleanup_unnecessary_files(dir: &std::path::PathBuf) {
    let unnecessary_extensions = [".zip", ".tgz", ".tar.gz", ".tar", ".gz"];

    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .for_each(|path| {
                    if path.is_file() {
                        process_cleanup_file(&path, &unnecessary_extensions);
                    } else if path.is_dir() {
                        cleanup_unnecessary_files(&path);
                        try_remove_empty_directory(&path);
                    }
                });
        })
        .unwrap_or(());
}

fn process_cleanup_file(path: &std::path::PathBuf, unnecessary_extensions: &[&str]) {
    if let Some(name) = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            unnecessary_extensions
                .iter()
                .any(|&ext| name.ends_with(ext))
        })
    {
        std::fs::remove_file(path)
            .map(|_| println!("   Cleaned up: {name}"))
            .unwrap_or_else(|e| eprintln!("Warning: Failed to remove {name}: {e}"))
    }
}

fn try_remove_empty_directory(path: &std::path::PathBuf) {
    let is_empty = std::fs::read_dir(path)
        .map(|entries| entries.count() == 0)
        .unwrap_or(false);

    if is_empty {
        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
            std::fs::remove_dir(path)
                .map(|_| println!("   Removed empty directory: {dir_name}"))
                .unwrap_or_else(|e| {
                    eprintln!("Warning: Failed to remove empty directory {dir_name}: {e}")
                })
        }
    }
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
pub async fn ensure_models_available() -> Result<()> {
    ensure_resources_available().await
}

pub async fn update_models_only() -> Result<()> {
    println!("🔄 Updating voice models only...");

    let target_dir = std::env::var("HOME")
        .ok()
        .map(|_| get_default_voicevox_dir())
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    std::fs::create_dir_all(&target_dir)?;

    let downloader_path = find_downloader_binary()?;

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Downloading voice models only...");

    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(&target_dir)
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            let vvm_count = count_vvm_files_recursive(&target_dir.join("models"));
            println!("✅ Voice models updated successfully!");
            println!("   Found {vvm_count} VVM model files");
            cleanup_unnecessary_files(&target_dir);
            Ok(())
        }
        _ => {
            println!("⚠️  Models-only update not supported, falling back to full update...");
            launch_downloader_for_user().await
        }
    }
}

pub async fn update_dictionary_only() -> Result<()> {
    println!("🔄 Updating dictionary only...");

    let target_dir = std::env::var("HOME")
        .ok()
        .map(|_| get_default_voicevox_dir())
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    std::fs::create_dir_all(&target_dir)?;

    let downloader_path = find_downloader_binary()?;

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Downloading dictionary only...");

    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("dict")
        .arg("--output")
        .arg(&target_dir)
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            println!("✅ Dictionary updated successfully!");
            cleanup_unnecessary_files(&target_dir);
            Ok(())
        }
        _ => {
            println!("⚠️  Dictionary-only update not supported, falling back to full update...");
            launch_downloader_for_user().await
        }
    }
}

pub async fn update_specific_model(model_id: u32) -> Result<()> {
    println!("🔄 Updating model {model_id} only...");

    let target_dir = std::env::var("HOME")
        .ok()
        .map(|_| get_default_voicevox_dir())
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    std::fs::create_dir_all(&target_dir)?;

    let downloader_path = find_downloader_binary()?;

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Downloading model {model_id} only...");

    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(&target_dir)
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            println!("✅ Model {model_id} updated successfully!");
            cleanup_unnecessary_files(&target_dir);
            Ok(())
        }
        _ => {
            println!("⚠️  Specific model update not supported, falling back to full update...");
            launch_downloader_for_user().await
        }
    }
}

pub async fn check_updates() -> Result<()> {
    println!("🔍 Checking for available updates...");

    use crate::voice::scan_available_models;
    let current_models = scan_available_models()?;

    println!("📊 Current installation status:");
    println!("  Voice models: {} VVM files", current_models.len());
    for model in &current_models {
        println!(
            "    Model {} ({})",
            model.model_id,
            model.file_path.display()
        );
    }

    use crate::paths::find_openjtalk_dict;
    match find_openjtalk_dict() {
        Ok(dict_path) => {
            println!("  Dictionary: {} ✅", dict_path.display());
        }
        Err(_) => {
            println!("  Dictionary: Not found ❌");
        }
    }

    println!();
    println!("💡 Update options:");
    println!("  --update-models     Update all voice models");
    println!("  --update-dict       Update dictionary only");
    println!("  --update-model N    Update specific model N");

    Ok(())
}

pub async fn show_version_info() -> Result<()> {
    println!("📋 VOICEVOX CLI Version Information");
    println!("=====================================");

    println!("Application: v{}", env!("CARGO_PKG_VERSION"));

    use crate::voice::scan_available_models;
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

    use crate::paths::find_openjtalk_dict;
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

fn get_file_modified(path: &PathBuf) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(format!("{modified:?}"))
}
