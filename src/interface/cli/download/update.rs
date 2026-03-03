use anyhow::Result;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

use super::{
    cleanup::{cleanup_unnecessary_files, count_vvm_files_recursive},
    default_download_target_dir, find_downloader_binary, launch_downloader_for_user,
};

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
            Self::Dictionary => println!(" Dictionary updated successfully!"),
            Self::SpecificModel(model_id) => println!(" Model {model_id} updated successfully!"),
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

pub async fn update_models_only() -> Result<()> {
    run_update_request(UpdateRequest::Models).await
}

pub async fn update_dictionary_only() -> Result<()> {
    run_update_request(UpdateRequest::Dictionary).await
}

pub async fn update_specific_model(model_id: u32) -> Result<()> {
    run_update_request(UpdateRequest::SpecificModel(model_id)).await
}
