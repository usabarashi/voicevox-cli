use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use crate::paths::get_default_models_dir;

pub fn attempt_first_run_setup() -> Result<PathBuf> {
    println!("🎭 VOICEVOX CLI - User Setup");
    println!("Setting up voice models for current user...");
    println!();

    let target_dir = get_default_models_dir();

    println!(
        "📦 Installing models to: {} (user-specific)",
        target_dir.display()
    );
    println!("   No sudo privileges required");

    let exe_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(_) => {
            return show_manual_setup_instructions(&target_dir);
        }
    };

    let pkg_root = match exe_path.parent().and_then(|p| p.parent()) {
        Some(root) => root,
        None => return show_manual_setup_instructions(&target_dir),
    };

    let auto_setup = pkg_root.join("bin/voicevox-auto-setup");
    if !auto_setup.exists() {
        return show_manual_setup_instructions(&target_dir);
    }

    println!("🔄 Running automatic setup...");

    let status = std::process::Command::new(&auto_setup)
        .arg(&target_dir)
        .status();

    match status {
        Ok(status) if status.success() => {
            if is_valid_models_directory(&target_dir) {
                return Ok(target_dir);
            }

            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    if !entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
                        continue;
                    }

                    let subdir = entry.path();
                    if is_valid_models_directory(&subdir) {
                        return Ok(subdir);
                    }
                }
            }

            println!("⚠️  Setup completed but no models found");
        }
        Ok(_) => {
            println!("⚠️  Automatic setup failed");
        }
        Err(e) => {
            println!("⚠️  Could not run automatic setup: {e}");
        }
    }

    show_manual_setup_instructions(&target_dir)
}

fn show_manual_setup_instructions(target_dir: &Path) -> Result<PathBuf> {
    println!();
    println!("📋 Manual Setup Required:");
    println!(
        "1. Run: voicevox-download --output {}",
        target_dir.display()
    );
    println!("2. Accept the VOICEVOX license terms");
    println!("3. Try running voicevox-say again");
    println!();
    println!("License Summary:");
    println!("- VOICEVOX voice models are free for commercial/non-commercial use");
    println!("- Credit required: 'VOICEVOX:[Character Name]' in generated audio");
    println!("- Full terms: https://voicevox.hiroshiba.jp/");

    Err(anyhow!(
        "Voice models not available. Please run setup manually."
    ))
}

pub fn is_valid_models_directory(path: &PathBuf) -> bool {
    fn find_vvm_files_recursive(dir: &PathBuf) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let entry_path = entry.path();

                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".vvm") {
                        return true;
                    }
                }

                if entry_path.is_dir() && find_vvm_files_recursive(&entry_path) {
                    return true;
                }
            }
        }
        false
    }

    find_vvm_files_recursive(path)
}
