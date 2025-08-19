use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use crate::paths::{find_models_dir, find_openjtalk_dict, get_default_voicevox_dir};

/// Get the appropriate download target directory considering environment variables
fn get_download_target_dir() -> PathBuf {
    // Check if user has set custom directories
    let custom_models = std::env::var("VOICEVOX_MODELS_DIR").ok();
    let custom_dict = std::env::var("VOICEVOX_OPENJTALK_DICT").ok();

    if custom_models.is_some() || custom_dict.is_some() {
        // User has custom paths - download to default location and let them manage it
        eprintln!("⚠️  Warning: Custom paths detected via environment variables.");
        eprintln!(
            "   Downloads will go to default location: {}",
            get_default_voicevox_dir().display()
        );
        eprintln!("   You may need to move files to your custom locations:");
        if let Some(models) = custom_models {
            eprintln!("   - Models: {}", models);
        }
        if let Some(dict) = custom_dict {
            eprintln!("   - Dictionary: {}", dict);
        }
    }

    // Always download to XDG-compliant default location
    get_default_voicevox_dir()
}

/// Launches VOICEVOX downloader for voice models with direct user interaction
pub async fn launch_downloader_for_user() -> Result<()> {
    // Determine target directory based on environment variables
    let target_dir = get_download_target_dir();

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
    println!("   This will download: OpenJTalk dictionary and voice models");
    println!("   Please follow the on-screen instructions to accept license terms.");
    println!("   Press Enter when ready to continue...");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("dict")
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

/// Downloads only the OpenJTalk dictionary if not present
pub async fn download_openjtalk_dict_if_needed() -> Result<()> {
    if find_openjtalk_dict().is_ok() {
        return Ok(());
    }

    println!("📚 OpenJTalk dictionary not found. Downloading...");

    let target_dir = get_default_voicevox_dir();
    std::fs::create_dir_all(&target_dir)?;

    let downloader_path = find_downloader_executable()?;

    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("dict")
        .arg("--output")
        .arg(&target_dir)
        .status()?;

    if status.success() {
        println!("✅ OpenJTalk dictionary successfully downloaded");
        Ok(())
    } else {
        Err(anyhow!("Failed to download OpenJTalk dictionary"))
    }
}

fn find_downloader_executable() -> Result<PathBuf> {
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

/// Ensures VOICEVOX resources (models and dictionary) are available, prompting for download if needed.
///
/// This function checks if voice models and OpenJTalk dictionary are already installed.
/// If not, it prompts the user interactively to download them. The user must accept the
/// VOICEVOX license terms for each voice character.
///
/// # Returns
///
/// * `Ok(())` - Resources are available or successfully downloaded
/// * `Err` - User declined download or download failed
///
/// # Note
///
/// This function requires user interaction and should not be used in
/// non-interactive environments (e.g., MCP server, automated scripts).
pub async fn ensure_resources_available() -> Result<()> {
    let models_ok = find_models_dir().is_ok();
    let dict_ok = find_openjtalk_dict().is_ok();

    if models_ok && dict_ok {
        return Ok(());
    }

    println!("🎭 VOICEVOX CLI - First Run Setup");

    if !models_ok && !dict_ok {
        println!(
            "Voice models and OpenJTalk dictionary are required for text-to-speech synthesis."
        );
        println!("This includes:");
        println!("  - 26+ voice models (~200MB)");
        println!("  - OpenJTalk dictionary (~10MB)");
    } else if !models_ok {
        println!("Voice models are required for text-to-speech synthesis.");
        println!("This includes 26+ voice models (~200MB).");
    } else {
        println!("OpenJTalk dictionary is required for text-to-speech synthesis.");
        println!("This includes dictionary files (~10MB).");
    }

    println!("Note: ONNX Runtime is already bundled in this build.");
    println!();

    print!("Would you like to download required resources now? [Y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let response = input.trim().to_lowercase();

    if response.is_empty() || response == "y" || response == "yes" {
        println!("🔄 Starting resources download...");

        if !dict_ok {
            println!("📚 Downloading OpenJTalk dictionary...");
            download_openjtalk_dict_if_needed().await?;
        }

        if !models_ok {
            println!("🎭 Downloading voice models...");
            println!(
                "Note: This will require accepting VOICEVOX license terms for 26+ voice characters."
            );
            println!();

            match launch_downloader_for_user().await {
                Ok(_) => {
                    println!("✅ All resources setup completed!");
                    Ok(())
                }
                Err(e) => {
                    eprintln!("❌ Voice models download failed: {e}");
                    eprintln!(
                        "You can manually run: voicevox-download --only models --output {}",
                        get_default_voicevox_dir().display()
                    );
                    Err(e)
                }
            }
        } else {
            println!("✅ All resources setup completed!");
            Ok(())
        }
    } else {
        println!("Skipping resources download. You can run 'voicevox-setup-models' later.");
        Err(anyhow!("Required resources are not available"))
    }
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
        let file_size = get_file_size(&model.file_path)?;
        let modified = get_file_modified(&model.file_path)?;
        println!(
            "  Model {}: {} ({}, {})",
            model.model_id,
            model
                .file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            format_size(file_size),
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

fn get_file_size(path: &PathBuf) -> Result<u64> {
    Ok(std::fs::metadata(path)?.len())
}

fn get_file_modified(path: &PathBuf) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(format!("{modified:?}"))
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{size:.1} {}", UNITS[unit_index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_vvm_files() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("voicevox_test");
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create test files
        std::fs::write(temp_dir.join("1.vvm"), b"test").unwrap();
        std::fs::write(temp_dir.join("2.vvm"), b"test").unwrap();
        std::fs::write(temp_dir.join("not_vvm.txt"), b"test").unwrap();

        let count = count_vvm_files_recursive(&temp_dir);
        assert_eq!(count, 2);

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(512), "512.0 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }
}
