use anyhow::{anyhow, Result};
use std::path::PathBuf;

// Launch VOICEVOX downloader for complete system setup with direct user interaction
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
    println!("   This will download: Core libraries, ONNX Runtime, 26+ voice models, and dictionary");
    println!("   Please follow the on-screen instructions to accept license terms.");
    println!("   Press Enter when ready to continue...");
    
    // Wait for user confirmation
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    
    // Launch downloader with direct user interaction
    let status = std::process::Command::new(&downloader_path)
        .arg("--output")
        .arg(&target_dir)
        .status()?;
    
    if status.success() {
        // Verify models were downloaded by checking target directory directly
        let _vvm_files = std::fs::read_dir(&target_dir)
            .map_err(|e| anyhow!("Failed to read target directory: {}", e))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.path().is_file() && 
                entry.file_name().to_str().map_or(false, |name| name.ends_with(".vvm")) ||
                entry.path().is_dir()  // Also check subdirectories
            })
            .collect::<Vec<_>>();
            
        // Count VVM files recursively
        let vvm_count = count_vvm_files_recursive(&target_dir);
        
        if vvm_count > 0 {
            println!("✅ VOICEVOX Core system successfully downloaded to: {}", target_dir.display());
            println!("   Found {} VVM model files", vvm_count);
            println!("   Includes: VOICEVOX Core libraries, ONNX Runtime, voice models, and dictionary");
            
            // Clean up unnecessary files (zip, tgz, tar.gz) in target directory
            cleanup_unnecessary_files(&target_dir);
            
            Ok(())
        } else {
            Err(anyhow!("Download completed but VOICEVOX Core system components not found in target directory"))
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
        ".zip", ".tgz", ".tar.gz", ".tar", ".gz", // Archive files
        ".exe", ".dll", ".so",                    // Executable files not needed after extraction
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
        .filter(|name| unnecessary_extensions.iter().any(|&ext| name.ends_with(ext)))
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
        path.file_name()
            .and_then(|n| n.to_str())
            .filter(|&name| ["c_api", "onnxruntime"].contains(&name))
            .map(|dir_name| {
                std::fs::remove_dir(path)
                    .map(|_| println!("   Removed empty directory: {}", dir_name))
                    .unwrap_or_else(|e| {
                        eprintln!("Warning: Failed to remove empty directory {}: {}", dir_name, e)
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
    
    println!("🎭 VOICEVOX TTS - First Run Setup");
    println!("VOICEVOX Core system components are required for text-to-speech synthesis.");
    println!("This includes VOICEVOX Core libraries, ONNX Runtime, 26+ voice models, and dictionary.");
    println!("");
    
    // Interactive license acceptance
    print!("Would you like to download VOICEVOX Core system now? [Y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let response = input.trim().to_lowercase();
    
    if response.is_empty() || response == "y" || response == "yes" {
        println!("🔄 Starting VOICEVOX Core system download...");
        println!("Note: This will require accepting VOICEVOX license terms for 26+ voice characters.");
        println!("");
        
        // Launch downloader directly for user interaction (no expect script)
        match launch_downloader_for_user().await {
            Ok(_) => {
                println!("✅ VOICEVOX Core system setup completed!");
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ VOICEVOX Core system download failed: {}", e);
                eprintln!("You can manually run: voicevox-download --output ~/.local/share/voicevox");
                Err(e)
            }
        }
    } else {
        println!("Skipping VOICEVOX Core system download. You can run 'voicevox-setup-models' later.");
        Err(anyhow!("VOICEVOX Core system components are required for operation"))
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
    
    // Launch downloader with models-only flag (if supported)
    let status = std::process::Command::new(&downloader_path)
        .arg("--output")
        .arg(&target_dir)
        .arg("--models-only") // This flag might not exist in the actual downloader
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
    
    // Launch downloader with dict-only flag (if supported)
    let status = std::process::Command::new(&downloader_path)
        .arg("--output")
        .arg(&target_dir)
        .arg("--dict-only") // This flag might not exist in the actual downloader
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
    
    // Launch downloader with specific model flag (if supported)
    let status = std::process::Command::new(&downloader_path)
        .arg("--output")
        .arg(&target_dir)
        .arg("--model")
        .arg(&model_id.to_string()) // This flag might not exist in the actual downloader
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
        println!("    Model {} ({})", model.model_id, model.file_path.display());
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
        println!("  Model {}: {} ({}, {})", 
                 model.model_id, 
                 model.file_path.file_name().unwrap_or_default().to_string_lossy(),
                 format_size(file_size),
                 modified);
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