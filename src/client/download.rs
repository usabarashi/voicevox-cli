use anyhow::{anyhow, Result};
use std::path::PathBuf;

// Launch VOICEVOX downloader for direct user interaction
pub async fn launch_downloader_for_user() -> Result<()> {
    let target_dir = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/voicevox/models"))
        .unwrap_or_else(|| PathBuf::from("./models"));
    
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
        let vvm_files = std::fs::read_dir(&target_dir)
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
                
                // Remove empty directories (like c_api, onnxruntime if empty)
                if let Ok(entries) = std::fs::read_dir(&path) {
                    if entries.count() == 0 {
                        if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                            if ["c_api", "onnxruntime"].contains(&dir_name) {
                                if let Err(e) = std::fs::remove_dir(&path) {
                                    eprintln!("Warning: Failed to remove empty directory {}: {}", dir_name, e);
                                } else {
                                    println!("   Removed empty directory: {}", dir_name);
                                }
                            }
                        }
                    }
                }
            }
        }
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