use anyhow::{anyhow, Result};
use std::path::PathBuf;

// Attempt first-run setup for voice models with automatic license acceptance
pub fn attempt_first_run_setup() -> Result<PathBuf> {
    println!("🎭 VOICEVOX CLI - User Setup");
    println!("Setting up voice models for current user...");
    println!("");

    // Primary target: user directory for user-specific setup
    let target_dir = std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".local/share/voicevox/models"))
        .unwrap_or_else(|| PathBuf::from("./models"));

    println!("📦 Installing models to: {} (user-specific)", target_dir.display());
    println!("   No sudo privileges required");

    // Try automatic setup with voicevox-auto-setup
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(pkg_root) = exe_path.parent().and_then(|p| p.parent()) {
            let auto_setup = pkg_root.join("bin/voicevox-auto-setup");
            if auto_setup.exists() {
                println!("🔄 Running automatic setup...");
                
                let status = std::process::Command::new(&auto_setup)
                    .arg(&target_dir)
                    .status();

                match status {
                    Ok(status) if status.success() => {
                        // Check if we now have valid models
                        if is_valid_models_directory(&target_dir) {
                            return Ok(target_dir);
                        }
                        
                        // Search subdirectories for VVM files
                        if let Ok(entries) = std::fs::read_dir(&target_dir) {
                            for entry in entries.filter_map(|e| e.ok()) {
                                if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                                    let subdir = entry.path();
                                    if is_valid_models_directory(&subdir) {
                                        return Ok(subdir);
                                    }
                                }
                            }
                        }
                    }
                    Ok(_) => {
                        println!("⚠️  Automatic setup failed");
                    }
                    Err(e) => {
                        println!("⚠️  Could not run automatic setup: {}", e);
                    }
                }
            }
        }
    }

    // Fallback to manual instructions
    println!("");
    println!("📋 Manual Setup Required:");
    println!("1. Run: voicevox-download --output {}", target_dir.display());
    println!("2. Accept the VOICEVOX license terms");
    println!("3. Try running voicevox-say again");
    println!("");
    println!("License Summary:");
    println!("- VOICEVOX voice models are free for commercial/non-commercial use");
    println!("- Credit required: 'VOICEVOX:[Character Name]' in generated audio");
    println!("- Full terms: https://voicevox.hiroshiba.jp/");

    Err(anyhow!(
        "Voice models not available. Please run setup manually."
    ))
}

// Helper function to validate models directory (recursive search for .vvm files)
pub fn is_valid_models_directory(path: &PathBuf) -> bool {
    fn find_vvm_files_recursive(dir: &PathBuf) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let entry_path = entry.path();
                
                // Check if it's a .vvm file
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".vvm") {
                        return true;
                    }
                }
                
                // If it's a directory, search recursively
                if entry_path.is_dir() {
                    if find_vvm_files_recursive(&entry_path) {
                        return true;
                    }
                }
            }
        }
        false
    }
    
    find_vvm_files_recursive(path)
}