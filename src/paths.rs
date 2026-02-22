use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

const APP_NAME: &str = "voicevox";
const MODELS_SUBDIR: &str = "models";
const VVM_SUBDIR: &str = "vvms";
const OPENJTALK_DICT_SUBDIR: &str = "openjtalk_dict";
const ONNXRUNTIME_SUBDIR: &str = "onnxruntime/lib";
const DICT_SUBDIR: &str = "dict";
const SOCKET_FILENAME: &str = "voicevox-daemon.sock";

fn xdg_app_data_dirs() -> [Option<PathBuf>; 3] {
    [
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(|p| PathBuf::from(p).join(APP_NAME)),
        dirs::data_local_dir().map(|d| d.join(APP_NAME)),
        dirs::home_dir().map(|h| h.join(".local/share").join(APP_NAME)),
    ]
}

fn existing_dir_from_env(var: &str) -> Option<PathBuf> {
    std::env::var(var)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.exists() && path.is_dir())
}

fn dir_contains_vvm_files(dir: &Path) -> bool {
    std::fs::read_dir(dir).ok().is_some_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext == "vvm")
        })
    })
}

fn preferred_models_dir(base_dir: &Path) -> Option<PathBuf> {
    let candidate = base_dir.join(MODELS_SUBDIR);
    if !(candidate.exists() && candidate.is_dir()) {
        return None;
    }

    let vvms_dir = candidate.join(VVM_SUBDIR);
    if vvms_dir.exists() && vvms_dir.is_dir() && dir_contains_vvm_files(&vvms_dir) {
        Some(vvms_dir)
    } else {
        Some(candidate)
    }
}

fn has_extension_ignore_ascii_case(filename: &str, expected: &str) -> bool {
    Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(expected))
}

fn is_valid_onnxruntime_filename(filename: &str) -> bool {
    if cfg!(target_os = "macos") {
        filename == "libonnxruntime.dylib"
            || (filename.starts_with("libvoicevox_onnxruntime.")
                && has_extension_ignore_ascii_case(filename, "dylib"))
    } else if cfg!(target_os = "linux") {
        filename == "libonnxruntime.so"
            || (filename.starts_with("libvoicevox_onnxruntime.")
                && has_extension_ignore_ascii_case(filename, "so"))
    } else {
        filename == "onnxruntime.dll"
            || filename == "libonnxruntime.dll"
            || (filename.starts_with("libvoicevox_onnxruntime.")
                && has_extension_ignore_ascii_case(filename, "dll"))
    }
}

/// Get the default VOICEVOX data directory path using XDG Base Directory specification
/// Priority: $`XDG_DATA_HOME/voicevox` > ~/.local/share/voicevox
#[must_use]
pub fn get_default_voicevox_dir() -> PathBuf {
    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg_data_home).join(APP_NAME);
    }

    dirs::data_local_dir().map_or_else(
        || {
            dirs::home_dir().map_or_else(
                || PathBuf::from(".").join(APP_NAME),
                |h| h.join(".local/share").join(APP_NAME),
            )
        },
        |d| d.join(APP_NAME),
    )
}

#[must_use]
pub fn get_socket_path() -> PathBuf {
    if let Ok(path) = std::env::var("VOICEVOX_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    if let Ok(path) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(path).join(SOCKET_FILENAME);
    }

    if let Ok(path) = std::env::var("XDG_STATE_HOME") {
        return PathBuf::from(path).join(SOCKET_FILENAME);
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local/state")
            .join(SOCKET_FILENAME);
    }

    dirs::state_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(SOCKET_FILENAME)
}

/// Finds the VOICEVOX models directory, honoring environment overrides first.
///
/// # Errors
///
/// Returns an error if no plausible models directory can be found.
pub fn find_models_dir() -> Result<PathBuf> {
    let env_model_paths = [
        "VOICEVOX_MODELS_DIR",
        "VOICEVOX_MODEL_DIR",
        "VOICEVOX_MODELS_PATH",
        "VOICEVOX_MODEL_PATH",
        "VOICEVOX_MODELS",
    ];

    if let Some(models_dir) = env_model_paths.into_iter().find_map(existing_dir_from_env) {
        return Ok(models_dir);
    }

    // Search directories following XDG Base Directory specification
    let search_dirs: Vec<_> = xdg_app_data_dirs().into_iter().flatten().collect();

    if let Some(models_dir) = search_dirs.iter().find_map(|dir| preferred_models_dir(dir)) {
        return Ok(models_dir);
    }

    if let Some(dir) = search_dirs
        .into_iter()
        .find(|dir| dir.exists() && dir.is_dir())
    {
        return Ok(dir);
    }

    Err(anyhow!(
        "Models directory not found. Please run 'voicevox-setup' or set VOICEVOX_MODELS_DIR environment variable."
    ))
}

/// Finds the models directory with a more permissive client-side fallback.
///
/// # Errors
///
/// Returns an error only if fallback path construction fails unexpectedly.
pub fn find_models_dir_client() -> Result<PathBuf> {
    find_models_dir().or_else(|_| {
        // Use XDG Base Directory for client fallback
        let base_dir = get_default_voicevox_dir();
        let default_path = base_dir.join(MODELS_SUBDIR);

        if base_dir.exists() && base_dir.is_dir() {
            Ok(base_dir)
        } else {
            Ok(default_path)
        }
    })
}

/// Finds the `OpenJTalk` dictionary directory used by VOICEVOX.
///
/// # Errors
///
/// Returns an error if no installed dictionary can be located.
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

    for dir in xdg_app_data_dirs().into_iter().flatten() {
        // Check the old location first for backward compatibility
        let dict_path = dir.join(OPENJTALK_DICT_SUBDIR);
        if dict_path.exists() && dict_path.is_dir() {
            return Ok(dict_path);
        }

        // Check the new location used by voicevox-download
        let dict_dir = dir.join(DICT_SUBDIR);
        if dict_dir.exists() && dict_dir.is_dir() {
            // Look for open_jtalk_dic_* directories
            if let Ok(entries) = std::fs::read_dir(&dict_dir) {
                if let Some(path) = entries.flatten().map(|entry| entry.path()).find(|path| {
                    path.is_dir()
                        && path.file_name().is_some_and(|name| {
                            name.to_string_lossy().starts_with("open_jtalk_dic_")
                        })
                }) {
                    return Ok(path);
                }
            }
        }
    }

    Err(anyhow!(
        "OpenJTalk dictionary not found. Please run 'voicevox-setup' to download required resources, \
         or set VOICEVOX_OPENJTALK_DICT environment variable"
    ))
}

/// Helper function to find ONNX Runtime libraries in a directory
fn find_onnx_libraries_in_dir(lib_dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut candidates = std::fs::read_dir(lib_dir)
        .ok()
        .into_iter()
        .flat_map(std::iter::Iterator::flatten)
        .map(|entry| entry.path())
        .filter_map(|path| {
            let filename = path.file_name()?.to_string_lossy().into_owned();
            (path.is_file() && is_valid_onnxruntime_filename(&filename)).then(|| {
                let is_original = filename.starts_with("libvoicevox_onnxruntime.");
                (path, is_original)
            })
        })
        .collect::<Vec<_>>();

    // Sort to prioritize original voicevox libraries over symlinks
    // After fixing the rpath, the original library should work directly
    candidates.sort_by_key(|(_, is_original)| !*is_original);
    candidates
}

fn first_onnx_library_in(lib_dir: &Path) -> Option<PathBuf> {
    lib_dir
        .exists()
        .then(|| find_onnx_libraries_in_dir(lib_dir))
        .and_then(|candidates| candidates.into_iter().next())
        .map(|(path, _)| path)
}

/// Finds the ONNX Runtime dynamic library path used by VOICEVOX Core.
///
/// # Errors
///
/// Returns an error if no valid ONNX Runtime library candidate can be found.
pub fn find_onnxruntime() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("ORT_DYLIB_PATH") {
        let lib_path = PathBuf::from(path);
        if lib_path.exists() {
            // Security validation for ORT_DYLIB_PATH
            if let Some(filename) = lib_path.file_name() {
                let filename_str = filename.to_string_lossy();
                if is_valid_onnxruntime_filename(&filename_str) {
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
                }
            }
        }
    }

    if let Some(path) = xdg_app_data_dirs()
        .into_iter()
        .flatten()
        .map(|dir| dir.join(ONNXRUNTIME_SUBDIR))
        .find_map(|lib_dir| first_onnx_library_in(&lib_dir))
    {
        return Ok(path);
    }

    let system_paths = ["/usr/local/share/voicevox/lib", "/opt/voicevox/lib"];

    if let Some(path) = system_paths
        .into_iter()
        .map(Path::new)
        .find_map(first_onnx_library_in)
    {
        return Ok(path);
    }

    Err(anyhow!(
        "ONNX Runtime library not found. Please run 'voicevox-setup' to download required resources, \
         or set ORT_DYLIB_PATH environment variable"
    ))
}
