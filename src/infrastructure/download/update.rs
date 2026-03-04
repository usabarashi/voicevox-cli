use anyhow::{Result, bail};
use std::path::PathBuf;

use super::{
    cleanup::count_vvm_files_recursive, default_download_target_dir, find_downloader_binary,
    install::launch_models_downloader,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateKind {
    Models,
    Dictionary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateOutcome {
    pub kind: UpdateKind,
    pub target_dir: PathBuf,
    pub model_count: Option<usize>,
    pub used_fallback: bool,
}

impl UpdateKind {
    const fn resource(self) -> &'static str {
        match self {
            Self::Models => "models",
            Self::Dictionary => "dict",
        }
    }
}

async fn try_run_downloader_only(resource: &str, target_dir: &std::path::Path) -> Result<bool> {
    let status = tokio::process::Command::new(find_downloader_binary()?)
        .arg("--only")
        .arg(resource)
        .arg("--output")
        .arg(target_dir)
        .status()
        .await?;

    Ok(status.success())
}

async fn run_update(kind: UpdateKind) -> Result<UpdateOutcome> {
    let target_dir = default_download_target_dir();
    tokio::fs::create_dir_all(&target_dir).await?;

    if try_run_downloader_only(kind.resource(), &target_dir).await? {
        let model_count = match kind {
            UpdateKind::Models => {
                let count = count_vvm_files_recursive(&target_dir);
                if count == 0 {
                    bail!("Model update succeeded but no VVM files were produced");
                }
                Some(count)
            }
            _ => None,
        };
        return Ok(UpdateOutcome {
            kind,
            target_dir,
            model_count,
            used_fallback: false,
        });
    }

    match kind {
        UpdateKind::Dictionary => {
            bail!("Dictionary update failed and no fallback is available")
        }
        UpdateKind::Models => {
            let model_count = launch_models_downloader(&target_dir).await?;
            Ok(UpdateOutcome {
                kind,
                target_dir,
                model_count: Some(model_count),
                used_fallback: true,
            })
        }
    }
}

pub async fn update_models_only() -> Result<UpdateOutcome> {
    run_update(UpdateKind::Models).await
}

pub async fn update_dictionary_only() -> Result<UpdateOutcome> {
    run_update(UpdateKind::Dictionary).await
}
