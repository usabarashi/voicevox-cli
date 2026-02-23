use anyhow::Result;
use std::path::Path;

use crate::paths::find_openjtalk_dict;
use crate::voice::scan_available_models;

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
        Ok(dict_path) => println!("  Dictionary: {}", dict_path.display()),
        Err(_) => println!("  Dictionary: Not found"),
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
        Ok(dict_path) => println!("Dictionary: {}", dict_path.display()),
        Err(_) => println!("Dictionary: Not installed"),
    }

    Ok(())
}

fn get_file_modified(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(format!("{modified:?}"))
}
