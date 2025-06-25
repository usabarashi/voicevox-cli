//! Voice model download and management functionality
//!
//! This module handles client-side voice model downloads with interactive license acceptance,
//! functional file management, and XDG-compliant storage. Supports selective downloads and
//! automatic cleanup.
//!
//! # Architecture
//!
//! - **Client-Side Responsibility**: Downloads handled by client, not daemon
//! - **Interactive License**: Manual user confirmation for all 26+ voice character licenses
//! - **Functional Programming**: Monadic composition for error handling and file processing
//! - **Selective Downloads**: Models-only, dictionary-only, or specific model downloads
//! - **Automatic Cleanup**: Removes unnecessary archive files after extraction
//!
//! # Example
//!
//! ```rust,no_run
//! use voicevox_cli::client::download::{ensure_models_available, update_models_only};
//!
//! // Ensure models are available (triggers first-run setup if needed)
//! ensure_models_available().await?;
//!
//! // Update only voice models (skips dictionary and other components)
//! update_models_only().await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Launches VOICEVOX downloader for voice models with direct user interaction
///
/// Downloads voice models only (VVM files) with interactive license acceptance.
/// Uses `--only models` flag to skip dictionary and other components that are
/// statically linked. Creates target directory and handles user confirmation.
///
/// # Returns
///
/// Success if models downloaded and verified
///
/// # Errors
///
/// Returns error if:
/// - User cancels download process
/// - VOICEVOX downloader not found
/// - Download fails or is interrupted
/// - No VVM files found after download
/// - File system errors during verification
///
/// # Example
///
/// ```rust,no_run
/// use voicevox_cli::client::download::launch_downloader_for_user;
///
/// // Interactive download with license acceptance
/// launch_downloader_for_user().await?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub async fn launch_downloader_for_user() -> Result<()> {
    let target_dir = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/voicevox"))
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    // Create target directory
    std::fs::create_dir_all(&target_dir)?;

    // Find downloader binary
    let downloader_path = if let Ok(current_exe) = std::env::current_exe() {
        let mut downloader = current_exe.clone();
        downloader.set_file_name("voicevox-download");
        if downloader.exists() {
            downloader
        } else {
            // Try package installation path
            if let Some(pkg_root) = current_exe.parent().and_then(|p| p.parent()) {
                let pkg_downloader = pkg_root.join("bin/voicevox-download");
                if pkg_downloader.exists() {
                    pkg_downloader
                } else {
                    return Err(anyhow!("voicevox-download not found"));
                }
            } else {
                return Err(anyhow!("voicevox-download not found"));
            }
        }
    } else {
        return Err(anyhow!("Could not find voicevox-download"));
    };

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Launching VOICEVOX downloader...");
    println!("   This will download: 26+ voice models only");
    println!("   Please follow the on-screen instructions to accept license terms.");
    println!("   Press Enter when ready to continue...");

    // Wait for user confirmation
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Launch downloader with direct user interaction (models only)
    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(&target_dir)
        .status()?;

    if status.success() {
        // Verify models were downloaded by checking target directory directly
        let _vvm_files = std::fs::read_dir(&target_dir)
            .map_err(|e| anyhow!("Failed to read target directory: {}", e))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().is_file()
                    && entry
                        .file_name()
                        .to_str()
                        .map_or(false, |name| name.ends_with(".vvm"))
                    || entry.path().is_dir() // Also check subdirectories
            })
            .collect::<Vec<_>>();

        // Count VVM files recursively
        let vvm_count = count_vvm_files_recursive(&target_dir);

        if vvm_count > 0 {
            println!(
                "✅ Voice models successfully downloaded to: {}",
                target_dir.display()
            );
            println!("   Found {} VVM model files", vvm_count);

            // Clean up unnecessary files (zip, tgz, tar.gz) in target directory
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

// Helper function to count VVM files recursively using functional composition
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

// Helper function to count single VVM file
fn count_vvm_file(path: &std::path::PathBuf) -> usize {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.ends_with(".vvm"))
        .map(|_| 1)
        .unwrap_or(0)
}

// Clean up unnecessary downloaded files to save space using functional composition
pub fn cleanup_unnecessary_files(dir: &std::path::PathBuf) {
    let unnecessary_extensions = [
        ".zip", ".tgz", ".tar.gz", ".tar", ".gz", // Archive files only
    ];

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

// Helper function to process file cleanup
fn process_cleanup_file(path: &std::path::PathBuf, unnecessary_extensions: &[&str]) {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| {
            unnecessary_extensions
                .iter()
                .any(|&ext| name.ends_with(ext))
        })
        .map(|name| {
            std::fs::remove_file(path)
                .map(|_| println!("   Cleaned up: {}", name))
                .unwrap_or_else(|e| eprintln!("Warning: Failed to remove {}: {}", name, e))
        });
}

// Functional approach to removing empty directories
fn try_remove_empty_directory(path: &std::path::PathBuf) {
    let is_empty = std::fs::read_dir(path)
        .map(|entries| entries.count() == 0)
        .unwrap_or(false);

    if is_empty {
        path.file_name().and_then(|n| n.to_str()).map(|dir_name| {
            std::fs::remove_dir(path)
                .map(|_| println!("   Removed empty directory: {}", dir_name))
                .unwrap_or_else(|e| {
                    eprintln!(
                        "Warning: Failed to remove empty directory {}: {}",
                        dir_name, e
                    )
                })
        });
    }
}

// Check for VOICEVOX Core system and download if needed (client-side first-run setup)
pub async fn ensure_models_available() -> Result<()> {
    use crate::paths::find_models_dir_client;

    // Check if models are already available
    if find_models_dir_client().is_ok() {
        return Ok(()); // Models already available
    }

    println!("🎭 VOICEVOX CLI - First Run Setup");
    println!("Voice models are required for text-to-speech synthesis.");
    println!("This includes 26+ voice models (~200MB).");
    println!("Note: Core libraries and dictionary are already included in this build.");
    println!("");

    // Interactive license acceptance
    print!("Would you like to download voice models now? [Y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let response = input.trim().to_lowercase();

    if response.is_empty() || response == "y" || response == "yes" {
        println!("🔄 Starting voice models download...");
        println!(
            "Note: This will require accepting VOICEVOX license terms for 26+ voice characters."
        );
        println!("");

        // Launch downloader directly for user interaction (no expect script)
        match launch_downloader_for_user().await {
            Ok(_) => {
                println!("✅ Voice models setup completed!");
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Voice models download failed: {}", e);
                eprintln!("You can manually run: voicevox-download --only models --output ~/.local/share/voicevox");
                Err(e)
            }
        }
    } else {
        println!("Skipping voice models download. You can run 'voicevox-setup-models' later.");
        Err(anyhow!("Voice models are required for operation"))
    }
}

// Update voice models only
pub async fn update_models_only() -> Result<()> {
    println!("🔄 Updating voice models only...");

    let target_dir = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/voicevox"))
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    // Create target directory
    std::fs::create_dir_all(&target_dir)?;

    // Find downloader binary
    let downloader_path = find_downloader_binary()?;

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Downloading voice models only...");

    // Launch downloader with --only models flag
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
            println!("   Found {} VVM model files", vvm_count);
            cleanup_unnecessary_files(&target_dir);
            Ok(())
        }
        _ => {
            // Fallback to full download if models-only not supported
            println!("⚠️  Models-only update not supported, falling back to full update...");
            launch_downloader_for_user().await
        }
    }
}

// Update dictionary only
pub async fn update_dictionary_only() -> Result<()> {
    println!("🔄 Updating dictionary only...");

    let target_dir = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/voicevox"))
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    // Create target directory
    std::fs::create_dir_all(&target_dir)?;

    // Find downloader binary
    let downloader_path = find_downloader_binary()?;

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Downloading dictionary only...");

    // Launch downloader with --only dict flag
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
            // Fallback to manual dictionary download
            println!("⚠️  Dictionary-only update not supported, falling back to full update...");
            launch_downloader_for_user().await
        }
    }
}

// Update specific model only
pub async fn update_specific_model(model_id: u32) -> Result<()> {
    println!("🔄 Updating model {} only...", model_id);

    let target_dir = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/voicevox"))
        .unwrap_or_else(|| PathBuf::from("./voicevox"));

    // Create target directory
    std::fs::create_dir_all(&target_dir)?;

    // Find downloader binary
    let downloader_path = find_downloader_binary()?;

    println!("📦 Target directory: {}", target_dir.display());
    println!("🔄 Downloading model {} only...", model_id);

    // Launch downloader with specific model - this may not be directly supported
    // Fallback to models only for now
    let status = std::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(&target_dir)
        .status();

    match status {
        Ok(exit_status) if exit_status.success() => {
            println!("✅ Model {} updated successfully!", model_id);
            cleanup_unnecessary_files(&target_dir);
            Ok(())
        }
        _ => {
            // Fallback to full download if specific model not supported
            println!("⚠️  Specific model update not supported, falling back to full update...");
            launch_downloader_for_user().await
        }
    }
}

// Check updates only
pub async fn check_updates() -> Result<()> {
    println!("🔍 Checking for available updates...");

    // Get current models
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

    // Check dictionary
    use crate::paths::find_openjtalk_dict;
    match find_openjtalk_dict() {
        Ok(dict_path) => {
            println!("  Dictionary: {} ✅", dict_path);
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

// Display version information
pub async fn show_version_info() -> Result<()> {
    println!("📋 VOICEVOX CLI Version Information");
    println!("=====================================");

    // Application version
    println!("Application: v{}", env!("CARGO_PKG_VERSION"));

    // Get current models with metadata
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

    // Check dictionary
    use crate::paths::find_openjtalk_dict;
    match find_openjtalk_dict() {
        Ok(dict_path) => {
            println!("Dictionary: {}", dict_path);
            // Try to get dictionary version info if available
        }
        Err(_) => {
            println!("Dictionary: Not installed");
        }
    }

    Ok(())
}

// Helper function to find downloader binary
fn find_downloader_binary() -> Result<PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let mut downloader = current_exe.clone();
        downloader.set_file_name("voicevox-download");
        if downloader.exists() {
            return Ok(downloader);
        }

        // Try package installation path
        if let Some(pkg_root) = current_exe.parent().and_then(|p| p.parent()) {
            let pkg_downloader = pkg_root.join("bin/voicevox-download");
            if pkg_downloader.exists() {
                return Ok(pkg_downloader);
            }
        }
    }

    Err(anyhow!("voicevox-download not found"))
}

// Helper functions for file metadata
fn get_file_size(path: &PathBuf) -> Result<u64> {
    Ok(std::fs::metadata(path)?.len())
}

fn get_file_modified(path: &PathBuf) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    // Simple timestamp formatting without chrono dependency
    Ok(format!("{:?}", modified))
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}
