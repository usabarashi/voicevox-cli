use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

const MODELS_SUBDIR: &str = "models";
const VVM_SUBDIR: &str = "vvms";
const OPENJTALK_DICT_SUBDIR: &str = "openjtalk_dict";
const ONNXRUNTIME_SUBDIR: &str = "onnxruntime/lib";
const DICT_SUBDIR: &str = "dict";

fn xdg_app_data_dirs() -> [Option<PathBuf>; 3] {
    [
        std::env::var(crate::config::ENV_XDG_DATA_HOME)
            .ok()
            .map(|p| PathBuf::from(p).join(crate::config::APP_NAME)),
        dirs::data_local_dir().map(|d| d.join(crate::config::APP_NAME)),
        dirs::home_dir().map(|h| {
            h.join(crate::config::USER_LOCAL_SHARE_DIR)
                .join(crate::config::APP_NAME)
        }),
    ]
}

fn existing_dir_from_env(var: &str) -> Option<PathBuf> {
    std::env::var(var)
        .ok()
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
}

fn is_existing_dir(path: &Path) -> bool {
    path.is_dir()
}

fn dir_contains_vvm_files(dir: &Path) -> bool {
    std::fs::read_dir(dir).ok().is_some_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|filename| has_extension_ignore_ascii_case(filename, "vvm"))
        })
    })
}

fn preferred_models_dir(base_dir: &Path) -> Option<PathBuf> {
    let candidate = base_dir.join(MODELS_SUBDIR);
    candidate.is_dir().then(|| {
        let vvms_dir = candidate.join(VVM_SUBDIR);
        if vvms_dir.is_dir() && dir_contains_vvm_files(&vvms_dir) {
            vvms_dir
        } else {
            candidate
        }
    })
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
    std::env::var(crate::config::ENV_XDG_DATA_HOME)
        .ok()
        .map(|p| PathBuf::from(p).join(crate::config::APP_NAME))
        .or_else(|| dirs::data_local_dir().map(|d| d.join(crate::config::APP_NAME)))
        .or_else(|| {
            dirs::home_dir().map(|h| {
                h.join(crate::config::USER_LOCAL_SHARE_DIR)
                    .join(crate::config::APP_NAME)
            })
        })
        .unwrap_or_else(|| PathBuf::from(".").join(crate::config::APP_NAME))
}

#[must_use]
pub fn get_socket_path() -> PathBuf {
    std::env::var_os(crate::config::ENV_VOICEVOX_SOCKET_PATH)
        .map(PathBuf::from)
        .or_else(|| {
            [
                crate::config::ENV_XDG_RUNTIME_DIR,
                crate::config::ENV_XDG_STATE_HOME,
            ]
            .into_iter()
            .find_map(std::env::var_os)
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .map(|base| {
                base.join(crate::config::APP_NAME)
                    .join(crate::config::SOCKET_FILENAME)
            })
        })
        .or_else(|| {
            std::env::var_os(crate::config::ENV_HOME).map(|h| {
                PathBuf::from(h)
                    .join(crate::config::USER_LOCAL_STATE_DIR)
                    .join(crate::config::APP_NAME)
                    .join(crate::config::SOCKET_FILENAME)
            })
        })
        .unwrap_or_else(|| {
            dirs::state_dir()
                .unwrap_or_else(|| PathBuf::from(crate::config::DEFAULT_TMP_DIR))
                .join(crate::config::APP_NAME)
                .join(crate::config::SOCKET_FILENAME)
        })
}

/// Finds the VOICEVOX models directory, honoring environment overrides first.
///
/// # Errors
///
/// Returns an error if no plausible models directory can be found.
pub fn find_models_dir() -> Result<PathBuf> {
    let xdg_dirs = xdg_app_data_dirs();
    existing_dir_from_env(crate::config::ENV_VOICEVOX_MODELS_DIR)
        .or_else(|| {
            xdg_dirs
                .iter()
                .flatten()
                .find_map(|dir| preferred_models_dir(dir))
        })
        .or_else(|| {
            xdg_dirs
                .into_iter()
                .flatten()
                .find(|dir| is_existing_dir(dir))
        })
        .ok_or_else(|| {
            anyhow!(
                "Models directory not found. Please run 'voicevox-setup' or set VOICEVOX_MODELS_DIR environment variable."
            )
        })
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

        if base_dir.is_dir() {
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
    existing_dir_from_env(crate::config::ENV_VOICEVOX_OPENJTALK_DICT)
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
                .map(|exe_dir| exe_dir.join("../share/voicevox").join(OPENJTALK_DICT_SUBDIR))
                .filter(|path| is_existing_dir(path))
        })
        .or_else(|| {
            xdg_app_data_dirs()
                .into_iter()
                .flatten()
                .find_map(|dir| find_openjtalk_dict_in_xdg_dir(&dir))
        })
        .ok_or_else(|| {
            anyhow!(
                "OpenJTalk dictionary not found. Please run 'voicevox-setup' to download required resources, \
                 or set VOICEVOX_OPENJTALK_DICT environment variable"
            )
        })
}

fn find_openjtalk_dict_in_xdg_dir(dir: &Path) -> Option<PathBuf> {
    let legacy_dict = dir.join(OPENJTALK_DICT_SUBDIR);
    Some(legacy_dict)
        .filter(|p| is_existing_dir(p))
        .or_else(|| {
            std::fs::read_dir(dir.join(DICT_SUBDIR))
                .ok()
                .and_then(|entries| {
                    entries
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .find(|path| {
                            path.is_dir()
                                && path.file_name().is_some_and(|name| {
                                    name.to_string_lossy().starts_with("open_jtalk_dic_")
                                })
                        })
                })
        })
}

/// Helper function to find ONNX Runtime libraries in a directory
fn find_onnx_libraries_in_dir(lib_dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut candidates = std::fs::read_dir(lib_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
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
    candidates.sort_unstable_by_key(|(_, is_original)| !*is_original);
    candidates
}

fn first_onnx_library_in(lib_dir: &Path) -> Option<PathBuf> {
    lib_dir
        .exists()
        .then(|| find_onnx_libraries_in_dir(lib_dir))
        .and_then(|candidates| candidates.into_iter().next())
        .map(|(path, _)| path)
}

/// Validates ORT_DYLIB_PATH env var: checks file existence, filename validity,
/// and resolves symlinks.
fn validated_ort_dylib_path() -> Option<PathBuf> {
    std::env::var(crate::config::ENV_ORT_DYLIB_PATH)
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .filter(|p| {
            p.file_name()
                .and_then(|f| f.to_str())
                .is_some_and(is_valid_onnxruntime_filename)
        })
        .map(|p| std::fs::canonicalize(&p).unwrap_or(p))
        .filter(|p| p.is_file())
}

/// Finds the ONNX Runtime dynamic library path used by VOICEVOX Core.
///
/// # Errors
///
/// Returns an error if no valid ONNX Runtime library candidate can be found.
pub fn find_onnxruntime() -> Result<PathBuf> {
    validated_ort_dylib_path()
        .or_else(|| {
            xdg_app_data_dirs()
                .into_iter()
                .flatten()
                .map(|dir| dir.join(ONNXRUNTIME_SUBDIR))
                .find_map(|lib_dir| first_onnx_library_in(&lib_dir))
        })
        .or_else(|| {
            crate::config::SYSTEM_VOICEVOX_LIB_DIRS
                .into_iter()
                .map(Path::new)
                .find_map(first_onnx_library_in)
        })
        .ok_or_else(|| {
            anyhow!(
                "ONNX Runtime library not found. Please run 'voicevox-setup' to download required resources, \
                 or set ORT_DYLIB_PATH environment variable"
            )
        })
}
