use anyhow::{anyhow, Result};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{
    cleanup::{cleanup_incomplete_downloads, cleanup_unnecessary_files, count_vvm_files_recursive},
    collect_missing_resources, default_download_target_dir, find_downloader_binary,
};
use crate::paths::{find_onnxruntime, get_default_voicevox_dir};

fn print_missing_resource_summary(missing_resources: &[&str]) {
    println!("VOICEVOX CLI - Initial Setup Required");
    println!("The following resources need to be downloaded:");
    if missing_resources.contains(&"onnxruntime") {
        println!("  • ONNX Runtime - Neural network inference engine");
    }
    if missing_resources.contains(&"dict") {
        println!("  • OpenJTalk Dictionary - Japanese text processing");
    }
    if missing_resources.contains(&"models") {
        println!("  • Voice Models - Character voices");
    }
    println!();
}

async fn read_stdin_line() -> Result<String> {
    let mut input = String::new();
    let mut stdin = BufReader::new(tokio::io::stdin());
    stdin.read_line(&mut input).await?;
    Ok(input)
}

async fn prompt_for_resource_download() -> Result<bool> {
    print!("Would you like to download these resources now? [Y/n]: ");
    tokio::io::AsyncWriteExt::flush(&mut tokio::io::stdout()).await?;

    let input = read_stdin_line().await?;
    let response = input.trim().to_lowercase();
    Ok(response.is_empty() || response == "y" || response == "yes")
}

async fn run_downloader_for_resources(
    downloader_path: &Path,
    missing_resources: &[&str],
    target_dir: &Path,
) -> Result<std::process::ExitStatus> {
    let mut cmd = tokio::process::Command::new(downloader_path);
    for resource in missing_resources {
        cmd.arg("--only").arg(resource);
    }

    cmd.arg("--output")
        .arg(target_dir)
        .status()
        .await
        .map_err(Into::into)
}

fn maybe_set_ort_dylib_path(missing_resources: &[&str]) {
    if missing_resources.contains(&"onnxruntime") {
        if let Ok(ort_path) = find_onnxruntime() {
            std::env::set_var("ORT_DYLIB_PATH", ort_path);
        }
    }
}

fn print_download_failure_summary(
    target_dir: &Path,
    missing_resources: &[&str],
    max_retries: u32,
    last_error: Option<&str>,
) {
    if let Some(error) = last_error {
        eprintln!(" Resource download failed after {max_retries} attempts: {error}");
    } else {
        eprintln!("Resource download failed after {max_retries} attempts");
    }

    let manual_cmd = missing_resources
        .iter()
        .map(|r| format!("--only {r}"))
        .collect::<Vec<_>>()
        .join(" ");
    eprintln!(
        "You can manually run: voicevox-download {} --output {}",
        manual_cmd,
        target_dir.display()
    );
}

async fn download_missing_resources(missing_resources: &[&str]) -> Result<()> {
    println!("Starting resource download...");
    let target_dir = get_default_voicevox_dir();
    tokio::fs::create_dir_all(&target_dir).await?;
    let downloader_path = find_downloader_binary()?;
    println!("Downloading to: {}", target_dir.display());

    let max_retries = 3;
    let mut last_error = None;

    for attempt in 1..=max_retries {
        if attempt > 1 {
            println!(" Retrying download... (Attempt {attempt}/{max_retries})");
            cleanup_incomplete_downloads(&target_dir);
        }

        match run_downloader_for_resources(&downloader_path, missing_resources, &target_dir).await {
            Ok(exit_status) if exit_status.success() => {
                println!("All resources downloaded successfully!");
                maybe_set_ort_dylib_path(missing_resources);
                return Ok(());
            }
            Ok(exit_status) => {
                last_error = Some(format!(
                    "Download failed with exit code: {:?}",
                    exit_status.code()
                ));
            }
            Err(error) => {
                last_error = Some(format!("Failed to execute downloader: {error}"));
            }
        }

        if attempt < max_retries {
            println!("⏳ Download failed, waiting 2 seconds before retry...");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    cleanup_incomplete_downloads(&target_dir);
    print_download_failure_summary(
        &target_dir,
        missing_resources,
        max_retries,
        last_error.as_deref(),
    );
    Err(anyhow!(
        "Failed to download required resources after {max_retries} attempts"
    ))
}

/// Ensures all runtime resources (ONNX Runtime, dictionary, models) are available.
///
/// # Errors
///
/// Returns an error if user input cannot be read, required directories cannot be created,
/// downloader execution fails, or the user declines resource installation.
pub async fn ensure_resources_available() -> Result<()> {
    let missing_resources = collect_missing_resources();
    if missing_resources.is_empty() {
        return Ok(());
    }

    print_missing_resource_summary(&missing_resources);
    if prompt_for_resource_download().await? {
        download_missing_resources(&missing_resources).await
    } else {
        println!("Setup cancelled. You can run 'voicevox-setup' later to download resources.");
        Err(anyhow!("Required resources are not available"))
    }
}

/// Launches VOICEVOX downloader for voice models with direct user interaction.
///
/// # Errors
///
/// Returns an error if the downloader binary cannot be found, user input cannot be read,
/// process execution fails, or no model files are found after download.
pub async fn launch_downloader_for_user() -> Result<()> {
    let target_dir = default_download_target_dir();
    tokio::fs::create_dir_all(&target_dir).await?;

    let downloader_path = find_downloader_binary()?;

    println!(" Target directory: {}", target_dir.display());
    println!(" Launching VOICEVOX downloader...");
    println!("   This will download: 26+ voice models only");
    println!("   Please follow the on-screen instructions to accept license terms.");
    println!("   Press Enter when ready to continue...");

    let _input = read_stdin_line().await?;

    let status = tokio::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(&target_dir)
        .status()
        .await?;

    if status.success() {
        let vvm_count = count_vvm_files_recursive(&target_dir);

        if vvm_count > 0 {
            println!(
                " Voice models successfully downloaded to: {}",
                target_dir.display()
            );
            println!("   Found {vvm_count} VVM model files");
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
