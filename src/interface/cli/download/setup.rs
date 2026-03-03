use anyhow::{anyhow, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::infrastructure::download::{
    default_models_download_target_dir, download_missing_resources, launch_models_downloader,
    missing_resource_descriptions,
};
use crate::interface::{AppOutput, StdAppOutput};

pub use crate::infrastructure::download::{
    cleanup_unnecessary_files, count_vvm_files_recursive, has_startup_resources,
    missing_startup_resources,
};

async fn read_stdin_line() -> Result<String> {
    let mut input = String::new();
    let mut stdin = BufReader::new(tokio::io::stdin());
    stdin.read_line(&mut input).await?;
    Ok(input)
}

async fn prompt_for_resource_download(output: &dyn AppOutput) -> Result<bool> {
    output.info("Would you like to download these resources now? [Y/n]: ");
    tokio::io::stdout().flush().await?;

    let input = read_stdin_line().await?;
    let response = input.trim().to_lowercase();
    Ok(response.is_empty() || response == "y" || response == "yes")
}

fn print_missing_resource_summary(missing_resources: &[&str], output: &dyn AppOutput) {
    output.info("VOICEVOX CLI - Initial Setup Required");
    output.info("The following resources need to be downloaded:");
    for line in missing_resource_descriptions(missing_resources) {
        output.info(&format!("  - {line}"));
    }
}

pub async fn ensure_resources_available() -> Result<()> {
    let output = StdAppOutput;
    ensure_resources_available_with_output(&output).await
}

pub async fn ensure_resources_available_with_output(output: &dyn AppOutput) -> Result<()> {
    let missing_resources = missing_startup_resources();
    if missing_resources.is_empty() {
        return Ok(());
    }

    print_missing_resource_summary(&missing_resources, output);
    if !prompt_for_resource_download(output).await? {
        output.info("Setup cancelled. You can run setup later to download resources.");
        return Err(anyhow!("Required resources are not available"));
    }

    output.info("Starting resource download...");
    output.info(&format!(
        "Downloading to: {}",
        crate::infrastructure::paths::get_default_voicevox_dir().display()
    ));
    download_missing_resources(&missing_resources).await
}

pub async fn ensure_models_available() -> Result<()> {
    ensure_resources_available().await
}

pub async fn launch_downloader_for_user() -> Result<()> {
    let output = StdAppOutput;
    launch_downloader_for_user_with_output(&output).await
}

pub async fn launch_downloader_for_user_with_output(output: &dyn AppOutput) -> Result<()> {
    let target_dir = default_models_download_target_dir();
    output.info(&format!("Target directory: {}", target_dir.display()));
    output.info("Launching VOICEVOX downloader for models...");

    let count = launch_models_downloader(&target_dir).await?;
    output.info(&format!(
        "Voice models downloaded successfully. Found {count} VVM files"
    ));
    cleanup_unnecessary_files(&target_dir);
    Ok(())
}
