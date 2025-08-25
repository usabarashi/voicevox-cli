use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ComponentType {
    Models,
    Dictionary,
    OnnxRuntime,
}

impl ComponentType {
    fn error_message(&self) -> anyhow::Error {
        match self {
            ComponentType::Models => {
                anyhow!("No VOICEVOX models found. Run voicevox-setup to install.")
            }
            ComponentType::Dictionary => {
                anyhow!("OpenJTalk dictionary not found. Run voicevox-setup to install.")
            }
            ComponentType::OnnxRuntime => {
                anyhow!("ONNX Runtime library not found. Run voicevox-setup to install.")
            }
        }
    }
}

fn validate_path(path: &std::path::Path, validation_file: Option<&str>) -> bool {
    if !path.exists() || !path.is_dir() {
        return false;
    }

    if let Some(file) = validation_file {
        let file_path = path.join(file);
        file_path.exists() && file_path.is_file()
    } else {
        true
    }
}

const MODELS_SUBDIR: &str = "models";
const SOCKET_FILENAME: &str = "voicevox-daemon.sock";

/// Common path search function with unified priority order
pub fn find_component_path(
    component: ComponentType,
    env_var: Option<&str>,
    xdg_subpath: &str,
    validation_file: Option<&str>,
) -> Result<PathBuf> {
    // 1. Environment variable (highest priority)
    if let Some(env_var) = env_var {
        if let Ok(path) = std::env::var(env_var) {
            let component_path = match component {
                ComponentType::OnnxRuntime => {
                    // For ONNX Runtime, the env var points to the parent dir, append /lib
                    PathBuf::from(path).join("lib")
                }
                _ => PathBuf::from(path),
            };
            if validate_path(&component_path, validation_file) {
                return Ok(component_path);
            }
        }
    }

    // 2. XDG-compliant path (standard user installation)
    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".local/share"));
    let xdg_path = xdg_data_home.join("voicevox").join(xdg_subpath);
    if validate_path(&xdg_path, validation_file) {
        return Ok(xdg_path);
    }

    Err(component.error_message())
}

/// Get the default VOICEVOX data directory path
pub fn get_default_voicevox_dir() -> PathBuf {
    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".local/share"));
    xdg_data_home.join("voicevox")
}

/// Get the default models directory path
pub fn get_default_models_dir() -> PathBuf {
    get_default_voicevox_dir().join(MODELS_SUBDIR)
}

pub fn get_socket_path() -> PathBuf {
    let env_socket_paths = [
        ("VOICEVOX_SOCKET_PATH", ""),
        ("XDG_RUNTIME_DIR", SOCKET_FILENAME),
        ("XDG_STATE_HOME", SOCKET_FILENAME),
        ("HOME", &format!(".local/state/{SOCKET_FILENAME}")),
    ];

    for (env_var, suffix) in &env_socket_paths {
        if let Ok(value) = std::env::var(env_var) {
            let path = PathBuf::from(&value);
            return if suffix.is_empty() {
                path
            } else {
                path.join(suffix)
            };
        }
    }

    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(SOCKET_FILENAME)
}

pub fn find_models_dir() -> Result<PathBuf> {
    find_component_path(
        ComponentType::Models,
        Some("VOICEVOX_MODELS_DIR"),
        "models",
        None,
    )
}

pub fn find_models_dir_client() -> Result<PathBuf> {
    match find_models_dir() {
        Ok(dir) => Ok(dir),
        Err(_) => {
            let default_path = dirs::data_local_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
                .join("voicevox")
                .join(MODELS_SUBDIR);

            let alternative_path = default_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| {
                    dirs::data_local_dir()
                        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
                        .join("voicevox")
                });

            if alternative_path.exists() && alternative_path.is_dir() {
                Ok(alternative_path)
            } else {
                Ok(default_path)
            }
        }
    }
}

fn find_onnx_library(lib_dir: &std::path::Path) -> Option<PathBuf> {
    if lib_dir.exists() && lib_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(lib_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    // Check if filename matches the pattern
                    #[cfg(target_os = "macos")]
                    let matches = file_name.starts_with("libvoicevox_onnxruntime")
                        && file_name.ends_with(".dylib");
                    #[cfg(target_os = "linux")]
                    let matches = file_name.starts_with("libvoicevox_onnxruntime")
                        && file_name.ends_with(".so");

                    if matches {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn find_dict_subdirectory(dict_base: &std::path::Path) -> Option<PathBuf> {
    if dict_base.exists() && dict_base.is_dir() {
        if let Ok(entries) = std::fs::read_dir(dict_base) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    // Check if directory starts with "open_jtalk_dic"
                    if dir_name.starts_with("open_jtalk_dic") {
                        // Verify sys.dic exists
                        if path.join("sys.dic").exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn find_openjtalk_dict() -> Result<PathBuf> {
    // 1. Environment variable (highest priority)
    if let Ok(path) = std::env::var("VOICEVOX_OPENJTALK_DICT") {
        let dict_path = PathBuf::from(path);
        if validate_path(&dict_path, Some("sys.dic")) {
            return Ok(dict_path);
        }
    }

    // 2. XDG-compliant path (standard user installation)
    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".local/share"));

    let voicevox_dir = xdg_data_home.join("voicevox");

    // Search for versioned dictionary directories in dict/
    let dict_base = voicevox_dir.join("dict");
    if let Some(dict_path) = find_dict_subdirectory(&dict_base) {
        return Ok(dict_path);
    }

    // Fallback to old path structure for backward compatibility
    let old_dict_path = voicevox_dir.join("openjtalk_dict");
    if validate_path(&old_dict_path, Some("sys.dic")) {
        return Ok(old_dict_path);
    }

    Err(ComponentType::Dictionary.error_message())
}

pub fn find_onnxruntime_lib() -> Result<PathBuf> {
    // 1. Environment variable (highest priority)
    if let Ok(path) = std::env::var("VOICEVOX_ONNXRUNTIME_DIR") {
        let lib_dir = PathBuf::from(path).join("lib");
        if let Some(lib_file) = find_onnx_library(&lib_dir) {
            return Ok(lib_file);
        }
    }

    // 2. XDG-compliant path (standard user installation)
    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".local/share"));

    let lib_dir = xdg_data_home
        .join("voicevox")
        .join("onnxruntime")
        .join("lib");
    if let Some(lib_file) = find_onnx_library(&lib_dir) {
        return Ok(lib_file);
    }

    Err(ComponentType::OnnxRuntime.error_message())
}

#[derive(Debug)]
pub struct ComponentStatus {
    pub name: &'static str,
    pub status: Result<String>,
    pub details: Vec<String>,
}

pub fn check_all_components() -> Vec<ComponentStatus> {
    let mut components = Vec::new();

    components.push(ComponentStatus {
        name: "ONNX Runtime",
        status: crate::core::VoicevoxCore::check_onnx_runtime().map(|_| "OK".to_string()),
        details: vec![],
    });

    let models_status = crate::voice::scan_available_models().and_then(|models| {
        if models.is_empty() {
            Err(anyhow!(
                "No VOICEVOX models found. Run voicevox-setup to install."
            ))
        } else {
            Ok((format!("{} files installed", models.len()), models))
        }
    });

    let mut model_details = vec![];
    let models_result = match models_status {
        Ok((status, models)) => {
            for model in &models {
                let model_info = match std::fs::metadata(&model.file_path) {
                    Ok(metadata) => {
                        let size_kb = metadata.len() / 1024;
                        let filename = model
                            .file_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy();
                        format!("  Model {}: {filename} ({size_kb} KB)", model.model_id)
                    }
                    Err(_) => {
                        format!("  Model {} ({})", model.model_id, model.file_path.display())
                    }
                };
                model_details.push(model_info);
            }
            Ok(status)
        }
        Err(e) => Err(e),
    };

    components.push(ComponentStatus {
        name: "Voice Models",
        status: models_result,
        details: model_details,
    });

    components.push(ComponentStatus {
        name: "Dictionary",
        status: find_openjtalk_dict().map(|path| path.display().to_string()),
        details: vec![],
    });

    components
}
