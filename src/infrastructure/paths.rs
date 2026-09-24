use anyhow::{Result, anyhow};
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
        // Linux follows the ELF soname convention: the real library is
        // `libvoicevox_onnxruntime.so.<version>`, and `<name>.so` is a link.
        filename == "libonnxruntime.so"
            || (filename.starts_with("libvoicevox_onnxruntime.")
                && (has_extension_ignore_ascii_case(filename, "so")
                    || is_versioned_linux_onnxruntime_filename(filename)))
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

/// A candidate ONNX Runtime library with the attributes used for ranking.
struct OnnxLibraryCandidate {
    path: PathBuf,
    /// `libvoicevox_onnxruntime.*` rather than a compatibility symlink.
    is_original: bool,
    version: Option<Vec<u64>>,
}

/// True for the Linux versioned library name `libvoicevox_onnxruntime.so.<version>`
/// (for example `libvoicevox_onnxruntime.so.1.23.2`), which ELF soname
/// conventions use there instead of the macOS `<version>.dylib` form.
fn is_versioned_linux_onnxruntime_filename(filename: &str) -> bool {
    filename
        .strip_prefix("libvoicevox_onnxruntime.so.")
        .is_some_and(is_numeric_version)
}

/// True when `text` is a dot-separated sequence of non-empty decimal numbers.
fn is_numeric_version(text: &str) -> bool {
    !text.is_empty()
        && text
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Extracts the numeric version from a VOICEVOX ONNX Runtime library filename.
///
/// Handles the macOS form `libvoicevox_onnxruntime.<version>.dylib` and the
/// Linux form `libvoicevox_onnxruntime.so.<version>` (and the `libonnxruntime.`
/// compatibility prefix for either); unversioned names yield `None`.
fn parse_onnxruntime_version(filename: &str) -> Option<Vec<u64>> {
    let stem = filename
        .strip_prefix("libvoicevox_onnxruntime.")
        .or_else(|| filename.strip_prefix("libonnxruntime."))?;
    // On Linux the version follows the `so` extension (`...so.<version>`),
    // whereas on macOS and Windows it precedes the extension.
    let stem = stem.strip_prefix("so.").unwrap_or(stem);
    let version = stem
        .split('.')
        .take_while(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        .map(str::parse::<u64>)
        .collect::<Result<Vec<u64>, _>>()
        .ok()?;
    (!version.is_empty()).then_some(version)
}

/// Ranks ONNX Runtime libraries so the best candidate comes first: originals
/// before compatibility symlinks, then the highest version, then a stable path
/// order.
fn find_onnx_libraries_in_dir(lib_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = std::fs::read_dir(lib_dir)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter_map(|path| {
            let filename = path.file_name()?.to_string_lossy().into_owned();
            (path.is_file() && is_valid_onnxruntime_filename(&filename)).then(|| {
                let is_original = filename.starts_with("libvoicevox_onnxruntime.");
                OnnxLibraryCandidate {
                    version: parse_onnxruntime_version(&filename),
                    is_original,
                    path,
                }
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        b.is_original
            .cmp(&a.is_original)
            .then_with(|| b.version.cmp(&a.version))
            .then_with(|| a.path.cmp(&b.path))
    });
    candidates
        .into_iter()
        .map(|candidate| candidate.path)
        .collect()
}

fn first_onnx_library_in(lib_dir: &Path) -> Option<PathBuf> {
    lib_dir
        .exists()
        .then(|| find_onnx_libraries_in_dir(lib_dir))
        .and_then(|candidates| candidates.into_iter().next())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Versioned ONNX Runtime library filename as VOICEVOX ships it on this
    /// platform. The version precedes the extension on macOS and Windows but
    /// follows the `so` extension on Linux.
    fn versioned_onnx_library(version: &str) -> String {
        if cfg!(target_os = "linux") {
            format!("libvoicevox_onnxruntime.so.{version}")
        } else if cfg!(target_os = "macos") {
            format!("libvoicevox_onnxruntime.{version}.dylib")
        } else {
            format!("libvoicevox_onnxruntime.{version}.dll")
        }
    }

    /// Unversioned ONNX Runtime library filename accepted on this platform.
    fn unversioned_onnx_library() -> &'static str {
        if cfg!(target_os = "linux") {
            "libonnxruntime.so"
        } else if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.dll"
        }
    }

    #[test]
    fn parses_version_from_onnxruntime_filenames() {
        assert_eq!(
            parse_onnxruntime_version("libvoicevox_onnxruntime.1.23.2.dylib"),
            Some(vec![1, 23, 2])
        );
        assert_eq!(
            parse_onnxruntime_version("libvoicevox_onnxruntime.so.1.23.2"),
            Some(vec![1, 23, 2])
        );
        assert_eq!(
            parse_onnxruntime_version("libvoicevox_onnxruntime.so.1"),
            Some(vec![1])
        );
        assert_eq!(
            parse_onnxruntime_version("libonnxruntime.1.17.3.so"),
            Some(vec![1, 17, 3])
        );
        assert_eq!(parse_onnxruntime_version("libonnxruntime.dylib"), None);
        assert_eq!(
            parse_onnxruntime_version("libvoicevox_onnxruntime.dylib"),
            None
        );
        assert_eq!(
            parse_onnxruntime_version("libvoicevox_onnxruntime.so"),
            None
        );
    }

    #[test]
    fn recognizes_versioned_linux_onnxruntime_filenames() {
        assert!(is_versioned_linux_onnxruntime_filename(
            "libvoicevox_onnxruntime.so.1.23.2"
        ));
        assert!(is_versioned_linux_onnxruntime_filename(
            "libvoicevox_onnxruntime.so.1"
        ));
        assert!(!is_versioned_linux_onnxruntime_filename(
            "libvoicevox_onnxruntime.so"
        ));
        assert!(!is_versioned_linux_onnxruntime_filename(
            "libvoicevox_onnxruntime.so.latest"
        ));
        assert!(!is_versioned_linux_onnxruntime_filename(
            "libvoicevox_onnxruntime.so.1.23.2.dylib"
        ));
    }

    #[test]
    fn picks_highest_version_original_regardless_of_directory_order() {
        let dir = tempfile::tempdir().expect("tempdir");
        let newest = versioned_onnx_library("1.10.0");
        for name in [
            versioned_onnx_library("1.9.0"),
            newest.clone(),
            versioned_onnx_library("1.2.0"),
            unversioned_onnx_library().to_owned(),
        ] {
            fs::write(dir.path().join(name), b"").expect("write candidate");
        }

        let chosen = first_onnx_library_in(dir.path()).expect("a candidate");
        assert_eq!(
            chosen.file_name().and_then(|name| name.to_str()),
            Some(newest.as_str())
        );
    }

    #[test]
    fn falls_back_to_unversioned_library() {
        let dir = tempfile::tempdir().expect("tempdir");
        let name = unversioned_onnx_library();
        fs::write(dir.path().join(name), b"").expect("write candidate");

        let chosen = first_onnx_library_in(dir.path()).expect("a candidate");
        assert_eq!(
            chosen.file_name().and_then(|name| name.to_str()),
            Some(name)
        );
    }
}
