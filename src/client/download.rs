use anyhow::{anyhow, Result};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};

use super::download_cleanup::cleanup_incomplete_downloads;
use crate::paths::{
    find_models_dir, find_onnxruntime, find_openjtalk_dict, get_default_voicevox_dir,
};
use crate::voice::scan_available_models;

pub use super::download_cleanup::{cleanup_unnecessary_files, count_vvm_files_recursive};

fn collect_missing_resources() -> Vec<&'static str> {
    [
        ("onnxruntime", find_onnxruntime().is_err()),
        ("dict", find_openjtalk_dict().is_err()),
        ("models", find_models_dir().is_err()),
    ]
    .into_iter()
    .filter_map(|(name, missing)| missing.then_some(name))
    .collect()
}

fn default_download_target_dir() -> PathBuf {
    std::env::var_os("HOME").map_or_else(
        || PathBuf::from("./voicevox"),
        |_| get_default_voicevox_dir(),
    )
}

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
            Err(e) => {
                last_error = Some(format!("Failed to execute downloader: {e}"));
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

async fn try_run_downloader_only(resource: &str, target_dir: &Path) -> Result<bool> {
    let status = tokio::process::Command::new(find_downloader_binary()?)
        .arg("--only")
        .arg(resource)
        .arg("--output")
        .arg(target_dir)
        .status()
        .await?;

    Ok(status.success())
}

async fn prepare_update_target_dir() -> Result<PathBuf> {
    let target_dir = default_download_target_dir();
    tokio::fs::create_dir_all(&target_dir).await?;
    println!(" Target directory: {}", target_dir.display());
    Ok(target_dir)
}

enum UpdateRequest {
    Models,
    Dictionary,
    SpecificModel(u32),
}

impl UpdateRequest {
    const fn resource(&self) -> &'static str {
        match self {
            Self::Models | Self::SpecificModel(_) => "models",
            Self::Dictionary => "dict",
        }
    }

    fn start_message(&self) -> Cow<'static, str> {
        match self {
            Self::Models => Cow::Borrowed(" Updating voice models only..."),
            Self::Dictionary => Cow::Borrowed(" Updating dictionary only..."),
            Self::SpecificModel(model_id) => {
                Cow::Owned(format!(" Updating model {model_id} only..."))
            }
        }
    }

    fn progress_message(&self) -> Cow<'static, str> {
        match self {
            Self::Models => Cow::Borrowed(" Downloading voice models only..."),
            Self::Dictionary => Cow::Borrowed(" Downloading dictionary only..."),
            Self::SpecificModel(model_id) => {
                Cow::Owned(format!(" Downloading model {model_id} only..."))
            }
        }
    }

    const fn fallback_message(&self) -> &'static str {
        match self {
            Self::Models => "  Models-only update not supported, falling back to full update...",
            Self::Dictionary => {
                "  Dictionary-only update not supported, falling back to full update..."
            }
            Self::SpecificModel(_) => {
                "  Specific model update not supported, falling back to full update..."
            }
        }
    }

    fn print_success(&self, target_dir: &Path) {
        match self {
            Self::Models => {
                let vvm_count = count_vvm_files_recursive(&target_dir.join("models"));
                println!(" Voice models updated successfully!");
                println!("   Found {vvm_count} VVM model files");
            }
            Self::Dictionary => {
                println!(" Dictionary updated successfully!");
            }
            Self::SpecificModel(model_id) => {
                println!(" Model {model_id} updated successfully!");
            }
        }
    }
}

async fn run_update_request(request: UpdateRequest) -> Result<()> {
    println!("{}", request.start_message());

    let target_dir = prepare_update_target_dir().await?;
    println!("{}", request.progress_message());

    if try_run_downloader_only(request.resource(), &target_dir).await? {
        request.print_success(&target_dir);
        cleanup_unnecessary_files(&target_dir);
        Ok(())
    } else {
        println!("{}", request.fallback_message());
        launch_downloader_for_user().await
    }
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

/// Find the voicevox-download binary
fn find_downloader_binary() -> Result<PathBuf> {
    if let Ok(current_exe) = std::env::current_exe() {
        let downloader = current_exe.with_file_name("voicevox-download");
        if downloader.exists() {
            return Ok(downloader);
        }

        if let Some(pkg_root) = current_exe.parent().and_then(|p| p.parent()) {
            let pkg_downloader = pkg_root.join("bin/voicevox-download");
            if pkg_downloader.exists() {
                return Ok(pkg_downloader);
            }
        }
    }

    Err(anyhow!("voicevox-download not found"))
}

/// Launches VOICEVOX downloader for voice models with direct user interaction
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

/// Ensures VOICEVOX voice models are available, prompting for download if needed.
///
/// This function now checks all resources (ONNX Runtime, dictionary, models) and
/// downloads any missing components. It replaces the previous models-only check.
///
/// # Returns
///
/// * `Ok(())` - All resources are available or successfully downloaded
/// * `Err` - User declined download or download failed
///
/// # Note
///
/// This function requires user interaction and should not be used in
/// non-interactive environments (e.g., MCP server, automated scripts).
///
/// # Errors
///
/// Returns an error if resource detection, user input, or downloads fail.
pub async fn ensure_models_available() -> Result<()> {
    ensure_resources_available().await
}

/// Attempts to update only voice models using `voicevox-download`.
///
/// # Errors
///
/// Returns an error if fallback full download also fails.
pub async fn update_models_only() -> Result<()> {
    run_update_request(UpdateRequest::Models).await
}

/// Attempts to update only the `OpenJTalk` dictionary using `voicevox-download`.
///
/// # Errors
///
/// Returns an error if fallback full download also fails.
pub async fn update_dictionary_only() -> Result<()> {
    run_update_request(UpdateRequest::Dictionary).await
}

/// Attempts to update a specific model, falling back to full model download if unsupported.
///
/// # Errors
///
/// Returns an error if directory setup fails or fallback download fails.
pub async fn update_specific_model(model_id: u32) -> Result<()> {
    run_update_request(UpdateRequest::SpecificModel(model_id)).await
}

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
        Ok(dict_path) => {
            println!("  Dictionary: {}", dict_path.display());
        }
        Err(_) => {
            println!("  Dictionary: Not found");
        }
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
        Ok(dict_path) => {
            println!("Dictionary: {}", dict_path.display());
        }
        Err(_) => {
            println!("Dictionary: Not installed");
        }
    }

    Ok(())
}

fn get_file_modified(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(format!("{modified:?}"))
}
