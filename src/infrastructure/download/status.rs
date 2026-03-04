use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::infrastructure::paths::find_openjtalk_dict;
use crate::infrastructure::voicevox::scan_available_models;

#[derive(Debug, Clone)]
pub struct UpdateStatus {
    pub models: Vec<crate::infrastructure::voicevox::AvailableModel>,
    pub dictionary_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionModelEntry {
    pub model_id: u32,
    pub file_name: String,
    pub modified: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    pub app_version: &'static str,
    pub models: Vec<VersionModelEntry>,
    pub dictionary_path: Option<PathBuf>,
}

pub fn collect_update_status() -> Result<UpdateStatus> {
    let models = scan_available_models()?;
    let dictionary_path = find_openjtalk_dict().ok();
    Ok(UpdateStatus {
        models,
        dictionary_path,
    })
}

pub fn collect_version_info() -> Result<VersionInfo> {
    let current_models = scan_available_models()?;
    let models = current_models
        .iter()
        .map(|model| {
            let modified = get_file_modified(&model.file_path)?;
            let file_name = model
                .file_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Ok(VersionModelEntry {
                model_id: model.model_id,
                file_name,
                modified,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(VersionInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        models,
        dictionary_path: find_openjtalk_dict().ok(),
    })
}

fn get_file_modified(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    let modified = metadata.modified()?;
    Ok(format!("{modified:?}"))
}
