use anyhow::Result;

use crate::infrastructure::download::{collect_update_status, collect_version_info};
use crate::interface::{AppOutput, StdAppOutput};

pub fn check_updates() -> Result<()> {
    let output = StdAppOutput;
    check_updates_with_output(&output)
}

pub fn check_updates_with_output(output: &dyn AppOutput) -> Result<()> {
    let status = collect_update_status()?;
    output.info("Checking current installation status...");
    output.info("Current installation status:");
    output.info(&format!(
        "  Voice models: {} VVM files",
        status.models.len()
    ));
    for model in &status.models {
        output.info(&format!(
            "    Model {} ({})",
            model.model_id,
            model.file_path.display()
        ));
    }
    match status.dictionary_path {
        Some(path) => output.info(&format!("  Dictionary: {}", path.display())),
        None => output.info("  Dictionary: Not found"),
    }
    output.info("Update options:");
    output.info("  --update-models     Update all voice models");
    output.info("  --update-dict       Update dictionary only");
    Ok(())
}

pub fn show_version_info() -> Result<()> {
    let output = StdAppOutput;
    show_version_info_with_output(&output)
}

pub fn show_version_info_with_output(output: &dyn AppOutput) -> Result<()> {
    let version = collect_version_info()?;
    output.info("VOICEVOX CLI Version Information");
    output.info("=====================================");
    output.info(&format!("Application: v{}", version.app_version));
    output.info(&format!("Voice Models: {} installed", version.models.len()));
    for model in &version.models {
        output.info(&format!(
            "  Model {}: {} ({})",
            model.model_id, model.file_name, model.modified
        ));
    }
    match version.dictionary_path {
        Some(path) => output.info(&format!("Dictionary: {}", path.display())),
        None => output.info("Dictionary: Not installed"),
    }
    Ok(())
}
