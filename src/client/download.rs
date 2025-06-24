use anyhow::{anyhow, Result};
use std::path::PathBuf;

// Launch VOICEVOX downloader for direct user interaction
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
            println!("✅ Models successfully downloaded to: {}", target_dir.display());
            println!("   Found {} VVM model files", vvm_count);
            
            // Clean up unnecessary files (zip, tgz, tar.gz) in target directory
            cleanup_unnecessary_files(&target_dir);
            
            Ok(())
        } else {
            Err(anyhow!("Download completed but no VVM models found in target directory"))
        }
    } else {
        Err(anyhow!("Download process failed or was cancelled"))
    }
}

// Helper function to count VVM files recursively
pub fn count_vvm_files_recursive(dir: &std::path::PathBuf) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".vvm") {
                        count += 1;
                    }
                }
            } else if path.is_dir() {
                count += count_vvm_files_recursive(&path);
            }
        }
    }
    count
}

// Clean up unnecessary downloaded files to save space
pub fn cleanup_unnecessary_files(dir: &std::path::PathBuf) {
    let unnecessary_extensions = [
        ".zip", ".tgz", ".tar.gz", ".tar", ".gz", // Archive files
        ".exe", ".dll", ".so",                    // Executable files not needed after extraction
    ];
    
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if unnecessary_extensions.iter().any(|&ext| name.ends_with(ext)) {
                        if let Err(e) = std::fs::remove_file(&path) {
                            eprintln!("Warning: Failed to remove {}: {}", name, e);
                        } else {
                            println!("   Cleaned up: {}", name);
                        }
                    }
                }
            } else if path.is_dir() {
                // Recursively clean subdirectories
                cleanup_unnecessary_files(&path);
                
                // Remove empty directories (flattened logic)
                try_remove_empty_directory(&path);
            }
        }
    }
}

// Flattened empty directory removal logic
fn try_remove_empty_directory(path: &std::path::PathBuf) {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    
    if entries.count() != 0 {
        return;
    }
    
    let dir_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) if ["c_api", "onnxruntime"].contains(&name) => name,
        _ => return,
    };
    
    match std::fs::remove_dir(path) {
        Ok(_) => println!("   Removed empty directory: {}", dir_name),
        Err(e) => eprintln!("Warning: Failed to remove empty directory {}: {}", dir_name, e),
    }
}

// Check for models and download if needed (client-side first-run setup)
pub async fn ensure_models_available() -> Result<()> {
    use crate::paths::find_models_dir_client;
    
    // Check if models are already available
    if find_models_dir_client().is_ok() {
        return Ok(()); // Models already available
    }
    
    println!("🎭 VOICEVOX TTS - First Run Setup");
    println!("Voice models are required for text-to-speech synthesis.");
    println!("");
    
    // Interactive license acceptance
    print!("Would you like to download voice models now? [Y/n]: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let response = input.trim().to_lowercase();
    
    if response.is_empty() || response == "y" || response == "yes" {
        println!("🔄 Starting voice model download...");
        println!("Note: This will require accepting VOICEVOX license terms.");
        println!("");
        
        // Launch downloader directly for user interaction (no expect script)
        match launch_downloader_for_user().await {
            Ok(_) => {
                println!("✅ Voice models setup completed!");
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ Model download failed: {}", e);
                eprintln!("You can manually run: voicevox-download --output ~/.local/share/voicevox/models");
                Err(e)
            }
        }
    } else {
        println!("Skipping model download. You can run 'voicevox-setup-models' later.");
        Err(anyhow!("Voice models are required for operation"))
    }
}

// 音声モデルのみ更新
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

// 辞書のみ更新
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

// 特定モデルのみ更新
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

// 更新確認のみ
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

// バージョン情報表示
pub async fn show_version_info() -> Result<()> {
    println!("📋 VOICEVOX TTS Version Information");
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