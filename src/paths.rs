use anyhow::{anyhow, Result};
use std::path::PathBuf;

const VOICEVOX_DATA_SUBDIR: &str = ".local/share/voicevox";
const MODELS_SUBDIR: &str = "models";
const VVM_SUBDIR: &str = "vvms";
const OPENJTALK_DICT_SUBDIR: &str = "openjtalk_dict";
const SOCKET_FILENAME: &str = "voicevox-daemon.sock";

const ONNXRUNTIME_LIB_SUBDIR: &str = "onnxruntime/lib";
const VOICEVOX_ROOT_DIR: &str = "voicevox";
const SYSTEM_SHARE_PREFIX: &str = "/usr/local/share";
const OPT_PREFIX: &str = "/opt";

#[cfg(target_os = "macos")]
const ONNXRUNTIME_LIB_NAME: &str = "libvoicevox_onnxruntime.dylib";
#[cfg(target_os = "linux")]
const ONNXRUNTIME_LIB_NAME: &str = "libvoicevox_onnxruntime.so";

/// Get the default VOICEVOX data directory path
pub fn get_default_voicevox_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(VOICEVOX_DATA_SUBDIR))
        .unwrap_or_else(|| PathBuf::from("."))
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
    let env_model_paths = [
        "VOICEVOX_MODELS_DIR",
        "VOICEVOX_MODEL_DIR",
        "VOICEVOX_MODELS_PATH",
        "VOICEVOX_MODEL_PATH",
        "VOICEVOX_MODELS",
    ];

    for env_var in &env_model_paths {
        if let Ok(path) = std::env::var(env_var) {
            let models_dir = PathBuf::from(path);
            if models_dir.exists() && models_dir.is_dir() {
                return Ok(models_dir);
            }
        }
    }

    let search_dirs = [
        dirs::data_local_dir()
            .map(|d| d.join("voicevox"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join(VOICEVOX_DATA_SUBDIR))
            .unwrap_or_default(),
    ];

    for dir in &search_dirs {
        let candidate = dir.join(MODELS_SUBDIR);
        if candidate.exists() && candidate.is_dir() {
            let vvms_dir = candidate.join(VVM_SUBDIR);
            if vvms_dir.exists() && vvms_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&vvms_dir) {
                    let has_vvm = entries.filter_map(Result::ok).any(|entry| {
                        entry
                            .path()
                            .extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext == "vvm")
                            .unwrap_or(false)
                    });
                    if has_vvm {
                        return Ok(vvms_dir);
                    }
                }
            }
            return Ok(candidate);
        }
    }

    for dir in &search_dirs {
        if dir.exists() && dir.is_dir() {
            return Ok(dir.clone());
        }
    }

    Err(anyhow!(
        "Models directory not found. Please set VOICEVOX_MODELS_DIR or place models in ~/{}/{}",
        VOICEVOX_DATA_SUBDIR,
        MODELS_SUBDIR
    ))
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

pub fn find_openjtalk_dict() -> Result<PathBuf> {
    if let Some(embedded_path) = option_env!("VOICEVOX_OPENJTALK_DICT_EMBEDDED") {
        let dict_path = PathBuf::from(embedded_path);
        if dict_path.exists() && dict_path.is_dir() {
            return Ok(dict_path);
        }
    }

    if let Ok(path) = std::env::var("VOICEVOX_OPENJTALK_DICT") {
        let dict_path = PathBuf::from(path);
        if dict_path.exists() && dict_path.is_dir() {
            return Ok(dict_path);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(exe_dir) = current_exe.parent() {
            let installed_path = exe_dir
                .join("../share/voicevox")
                .join(OPENJTALK_DICT_SUBDIR);
            if installed_path.exists() && installed_path.is_dir() {
                return Ok(installed_path);
            }
        }
    }
    if let Some(data_dir) = dirs::data_local_dir() {
        let user_dict_path = data_dir.join("voicevox").join(OPENJTALK_DICT_SUBDIR);
        if user_dict_path.exists() && user_dict_path.is_dir() {
            return Ok(user_dict_path);
        }
    }

    Err(anyhow!(
        "OpenJTalk dictionary not found. Please set VOICEVOX_OPENJTALK_DICT environment variable \
         or ensure the dictionary is installed at <binary>/../share/voicevox/openjtalk_dict"
    ))
}

pub fn find_onnxruntime_lib() -> Result<PathBuf> {
    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs::home_dir().unwrap_or_default().join(".local/share"));

    let search_paths = [
        xdg_data_home
            .join(VOICEVOX_ROOT_DIR)
            .join(ONNXRUNTIME_LIB_SUBDIR),
        PathBuf::from(SYSTEM_SHARE_PREFIX)
            .join(VOICEVOX_ROOT_DIR)
            .join(ONNXRUNTIME_LIB_SUBDIR),
        PathBuf::from(OPT_PREFIX)
            .join(VOICEVOX_ROOT_DIR)
            .join(ONNXRUNTIME_LIB_SUBDIR),
    ];

    for path in &search_paths {
        let dylib = path.join(ONNXRUNTIME_LIB_NAME);

        if dylib.exists() {
            return Ok(path.clone());
        }
    }

    Err(anyhow!(
        "ONNX Runtime library not found. Please run voicevox-setup to install it.\n\
         Expected locations:\n  - {}/{}/{}\n  - {}/{}/{}\n  - {}/{}/{}",
        xdg_data_home.display(),
        VOICEVOX_ROOT_DIR,
        ONNXRUNTIME_LIB_SUBDIR,
        SYSTEM_SHARE_PREFIX,
        VOICEVOX_ROOT_DIR,
        ONNXRUNTIME_LIB_SUBDIR,
        OPT_PREFIX,
        VOICEVOX_ROOT_DIR,
        ONNXRUNTIME_LIB_SUBDIR
    ))
}
