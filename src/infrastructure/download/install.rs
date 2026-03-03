use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use super::{
    cleanup::{cleanup_incomplete_downloads, cleanup_unnecessary_files, count_vvm_files_recursive},
    find_downloader_binary,
};
use crate::infrastructure::paths::get_default_voicevox_dir;

pub fn missing_resource_descriptions(missing_resources: &[&str]) -> Vec<&'static str> {
    let mut descriptions = Vec::new();
    if missing_resources.contains(&"onnxruntime") {
        descriptions.push("ONNX Runtime - Neural network inference engine");
    }
    if missing_resources.contains(&"dict") {
        descriptions.push("OpenJTalk Dictionary - Japanese text processing");
    }
    if missing_resources.contains(&"models") {
        descriptions.push("Voice Models - Character voices");
    }
    descriptions
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

pub async fn download_missing_resources(missing_resources: &[&str]) -> Result<()> {
    if missing_resources.is_empty() {
        return Ok(());
    }

    let target_dir = get_default_voicevox_dir();
    tokio::fs::create_dir_all(&target_dir).await?;
    let downloader_path = find_downloader_binary()?;

    let max_retries = 3;
    let mut last_error = None;

    for attempt in 1..=max_retries {
        if attempt > 1 {
            cleanup_incomplete_downloads(&target_dir);
        }

        match run_downloader_for_resources(&downloader_path, missing_resources, &target_dir).await {
            Ok(exit_status) if exit_status.success() => {
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
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }

    cleanup_incomplete_downloads(&target_dir);
    let details = last_error.unwrap_or_else(|| "unknown error".to_string());
    Err(anyhow!(
        "Failed to download required resources after {max_retries} attempts: {details}"
    ))
}

pub async fn launch_models_downloader(target_dir: &Path) -> Result<usize> {
    tokio::fs::create_dir_all(target_dir).await?;
    let downloader_path = find_downloader_binary()?;

    let status = tokio::process::Command::new(&downloader_path)
        .arg("--only")
        .arg("models")
        .arg("--output")
        .arg(target_dir)
        .status()
        .await?;

    if !status.success() {
        return Err(anyhow!("Download process failed or was cancelled"));
    }

    let vvm_count = count_vvm_files_recursive(target_dir);
    if vvm_count == 0 {
        return Err(anyhow!(
            "Download completed but voice model files were not found in target directory"
        ));
    }

    cleanup_unnecessary_files(target_dir);
    Ok(vvm_count)
}

pub fn default_models_download_target_dir() -> PathBuf {
    super::default_download_target_dir()
}
