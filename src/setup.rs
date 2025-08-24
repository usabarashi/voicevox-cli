use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::paths::get_default_models_dir;

pub fn attempt_first_run_setup() -> Result<PathBuf> {
    println!("VOICEVOX CLI - User Setup");
    println!("Setting up voice models for current user...");
    println!();

    let target_dir = get_default_models_dir();

    println!(
        "Installing models to: {} (user-specific)",
        target_dir.display()
    );
    println!("   No sudo privileges required");

    show_manual_setup_instructions(&target_dir)
}

fn show_manual_setup_instructions(target_dir: &Path) -> Result<PathBuf> {
    println!();
    println!("Manual Setup Required:");
    println!(
        "1. Run: voicevox-setup to download models to {}",
        target_dir.display()
    );
    println!("2. Accept the VOICEVOX license terms");
    println!("3. Try running voicevox-say again");
    println!();
    println!("License Summary:");
    println!("- VOICEVOX voice models are free for commercial/non-commercial use");
    println!("- Credit required: 'VOICEVOX:[Character Name]' in generated audio");
    println!("- Full terms: https://voicevox.hiroshiba.jp/");

    Err(crate::daemon::DaemonError::NoModelsAvailable.into())
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
