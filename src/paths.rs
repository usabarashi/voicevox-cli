use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

const APP_NAME: &str = "voicevox";
const MODELS_SUBDIR: &str = "models";
const VVM_SUBDIR: &str = "vvms";
const OPENJTALK_DICT_SUBDIR: &str = "openjtalk_dict";
const ONNXRUNTIME_SUBDIR: &str = "lib";
const SOCKET_FILENAME: &str = "voicevox-daemon.sock";

/// Get the default VOICEVOX data directory path using XDG Base Directory specification
/// Priority: $XDG_DATA_HOME/voicevox > ~/.local/share/voicevox
pub fn get_default_voicevox_dir() -> PathBuf {
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg_data_home).join(APP_NAME);
    }

    dirs::data_local_dir()
        .map(|d| d.join(APP_NAME))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".local/share").join(APP_NAME))
                .unwrap_or_else(|| PathBuf::from(".").join(APP_NAME))
        })
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

    // Search directories following XDG Base Directory specification
    let mut search_dirs = Vec::new();

    // Priority 1: XDG_DATA_HOME/voicevox
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        search_dirs.push(PathBuf::from(xdg_data_home).join(APP_NAME));
    }

    // Priority 2: Standard XDG data directory
    if let Some(data_dir) = dirs::data_local_dir() {
        search_dirs.push(data_dir.join(APP_NAME));
    }

    // Priority 3: Fallback to ~/.local/share/voicevox
    if let Some(home) = dirs::home_dir() {
        search_dirs.push(home.join(".local/share").join(APP_NAME));
    }

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
        "Models directory not found. Please run 'voicevox-setup' or set VOICEVOX_MODELS_DIR environment variable."
    ))
}

pub fn find_models_dir_client() -> Result<PathBuf> {
    match find_models_dir() {
        Ok(dir) => Ok(dir),
        Err(_) => {
            // Use XDG Base Directory for client fallback
            let base_dir = get_default_voicevox_dir();
            let default_path = base_dir.join(MODELS_SUBDIR);

            if base_dir.exists() && base_dir.is_dir() {
                Ok(base_dir)
            } else {
                Ok(default_path)
            }
        }
    }
}

pub fn find_openjtalk_dict() -> Result<PathBuf> {
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

    let search_dirs = [
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(|p| PathBuf::from(p).join(APP_NAME)),
        dirs::data_local_dir().map(|d| d.join(APP_NAME)),
        dirs::home_dir().map(|h| h.join(".local/share").join(APP_NAME)),
    ];

    for dir in search_dirs.iter().flatten() {
        let dict_path = dir.join(OPENJTALK_DICT_SUBDIR);
        if dict_path.exists() && dict_path.is_dir() {
            return Ok(dict_path);
        }
    }

    Err(anyhow!(
        "OpenJTalk dictionary not found. Please run 'voicevox-setup' to download required resources, \
         or set VOICEVOX_OPENJTALK_DICT environment variable"
    ))
}

/// Find ONNX Runtime library
pub fn find_onnxruntime() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("ORT_DYLIB_PATH") {
        let lib_path = PathBuf::from(path);
        if lib_path.exists() {
            // Security validation for ORT_DYLIB_PATH
            if let Some(filename) = lib_path.file_name() {
                let filename_str = filename.to_string_lossy();
                let is_valid = if cfg!(target_os = "macos") {
                    filename_str == "libonnxruntime.dylib"
                        || filename_str.starts_with("libvoicevox_onnxruntime.")
                            && filename_str.ends_with(".dylib")
                } else if cfg!(target_os = "linux") {
                    filename_str == "libonnxruntime.so"
                        || filename_str.starts_with("libvoicevox_onnxruntime.")
                            && filename_str.ends_with(".so")
                } else {
                    filename_str == "onnxruntime.dll"
                        || filename_str == "libonnxruntime.dll"
                        || (filename_str.starts_with("libvoicevox_onnxruntime.")
                            && filename_str.ends_with(".dll"))
                };

                if is_valid {
                    // Resolve symlinks and verify the resolved path exists
                    match std::fs::canonicalize(&lib_path) {
                        Ok(canonical_path) => {
                            if canonical_path.exists() {
                                return Ok(canonical_path);
                            }
                        }
                        Err(_) => {
                            return Ok(lib_path);
                        }
                    }
                } else {
                    let expected_patterns = if cfg!(target_os = "macos") {
                        "libonnxruntime.dylib or libvoicevox_onnxruntime.*.dylib"
                    } else if cfg!(target_os = "linux") {
                        "libonnxruntime.so or libvoicevox_onnxruntime.*.so"
                    } else {
                        "onnxruntime.dll, libonnxruntime.dll, or libvoicevox_onnxruntime.*.dll"
                    };
                    eprintln!(
                        "Warning: ORT_DYLIB_PATH points to unexpected filename: {}. Expected: {}",
                        filename_str, expected_patterns
                    );
                }
            }
        }
    }

    let search_dirs = [
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(|p| PathBuf::from(p).join(APP_NAME)),
        dirs::data_local_dir().map(|d| d.join(APP_NAME)),
        dirs::home_dir().map(|h| h.join(".local/share").join(APP_NAME)),
    ];

    for dir in search_dirs.iter().flatten() {
        let lib_dir = dir.join(ONNXRUNTIME_SUBDIR);
        if lib_dir.exists() {
            // Try to find ONNX Runtime library with different naming patterns
            if let Ok(entries) = std::fs::read_dir(&lib_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        let matches = if cfg!(target_os = "macos") {
                            filename_str == "libonnxruntime.dylib"
                                || (filename_str.starts_with("libvoicevox_onnxruntime.")
                                    && filename_str.ends_with(".dylib"))
                        } else if cfg!(target_os = "linux") {
                            filename_str == "libonnxruntime.so"
                                || (filename_str.starts_with("libvoicevox_onnxruntime.")
                                    && filename_str.ends_with(".so"))
                        } else {
                            filename_str == "onnxruntime.dll"
                                || filename_str == "libonnxruntime.dll"
                                || (filename_str.starts_with("libvoicevox_onnxruntime.")
                                    && filename_str.ends_with(".dll"))
                        };

                        if matches && path.is_file() {
                            return Ok(path);
                        }
                    }
                }
            }
        }
    }

    let system_paths = ["/usr/local/share/voicevox/lib", "/opt/voicevox/lib"];

    for path in &system_paths {
        let lib_dir = Path::new(path);
        if lib_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(lib_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(filename) = path.file_name() {
                        let filename_str = filename.to_string_lossy();
                        let matches = if cfg!(target_os = "macos") {
                            filename_str == "libonnxruntime.dylib"
                                || (filename_str.starts_with("libvoicevox_onnxruntime.")
                                    && filename_str.ends_with(".dylib"))
                        } else if cfg!(target_os = "linux") {
                            filename_str == "libonnxruntime.so"
                                || (filename_str.starts_with("libvoicevox_onnxruntime.")
                                    && filename_str.ends_with(".so"))
                        } else {
                            filename_str == "onnxruntime.dll"
                                || filename_str == "libonnxruntime.dll"
                                || (filename_str.starts_with("libvoicevox_onnxruntime.")
                                    && filename_str.ends_with(".dll"))
                        };

                        if matches && path.is_file() {
                            return Ok(path);
                        }
                    }
                }
            }
        }
    }

    Err(anyhow!(
        "ONNX Runtime library not found. Please run 'voicevox-setup' to download required resources, \
         or set ORT_DYLIB_PATH environment variable"
    ))
}
