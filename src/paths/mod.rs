use anyhow::{anyhow, Result};
use std::path::PathBuf;

// Socket path for IPC - user-specific for daemon isolation
pub fn get_socket_path() -> PathBuf {
    // Environment variables with their corresponding socket paths
    let env_socket_paths = [
        ("VOICEVOX_SOCKET_PATH", ""),
        ("XDG_RUNTIME_DIR", "voicevox-daemon.sock"),
        ("XDG_STATE_HOME", "voicevox-daemon.sock"),
        ("HOME", ".local/state/voicevox-daemon.sock"),
    ];
    
    // Try environment variable based paths using functional approach
    env_socket_paths
        .iter()
        .find_map(|(env_var, suffix)| {
            std::env::var(env_var).ok().map(|env_value| {
                let socket_path = if suffix.is_empty() {
                    PathBuf::from(env_value)
                } else {
                    PathBuf::from(env_value).join(suffix)
                };
                
                // Create parent directory if needed (except for direct override)
                if !suffix.is_empty() {
                    if let Some(parent) = socket_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                }
                
                socket_path
            })
        })
        .unwrap_or_else(|| {
            // Fallback: User-specific temp socket with UID (not PID)
            let user_id = unsafe { libc::getuid() };
            PathBuf::from("/tmp").join(format!("voicevox-daemon-{}.sock", user_id))
        })
}

// Helper function to find VVM models directory (daemon: with download attempt)
pub fn find_models_dir() -> Result<PathBuf> {
    let search_paths = build_models_search_paths();

    for path_option in search_paths.into_iter() {
        if path_option.exists() && crate::setup::is_valid_models_directory(&path_option) {
            // Silent operation - no output for successful directory discovery
            return Ok(path_option);
        }
    }

    // If no models directory found, attempt first-run setup
    crate::setup::attempt_first_run_setup()
}

// Helper function to find VVM models directory (client: no download attempt)
pub fn find_models_dir_client() -> Result<PathBuf> {
    let search_paths = build_models_search_paths();

    for path_option in search_paths.into_iter() {
        if path_option.exists() && crate::setup::is_valid_models_directory(&path_option) {
            // Silent operation - no output for successful directory discovery
            return Ok(path_option);
        }
    }

    // No download attempt - just return error
    Err(anyhow!(
        "Voice models not found. Please start voicevox-daemon to download models automatically."
    ))
}

fn build_models_search_paths() -> Vec<PathBuf> {
    // Environment variables with their corresponding paths
    let env_paths = [
        ("VOICEVOX_MODELS_DIR", ""),
        ("HOME", ".local/share/voicevox/models/vvms"),
        ("XDG_DATA_HOME", "voicevox/models"),
        ("HOME", ".local/share/voicevox/models"),
        ("HOME", ".voicevox/models"), // Legacy
    ];
    
    // Static system paths
    let static_paths = [
        "./voicevox_core/models",
        "./models",
        "./voicevox_models/models/vvms", // Nix development layout
        "/usr/local/share/voicevox/models",
        "/usr/share/voicevox/models",
        "/opt/voicevox/models",
        "/opt/homebrew/share/voicevox/models",
        "/Applications/VOICEVOX.app/Contents/Resources/models",
    ];
    
    // Build paths using functional composition
    let env_based_paths = env_paths
        .iter()
        .filter_map(|(env_var, suffix)| {
            std::env::var(env_var).ok().map(|env_value| {
                if suffix.is_empty() {
                    PathBuf::from(env_value)
                } else {
                    PathBuf::from(env_value).join(suffix)
                }
            })
        });
    
    let static_based_paths = static_paths
        .iter()
        .map(|p| PathBuf::from(p));
    
    let package_path = std::env::current_exe()
        .ok()
        .and_then(|exe_path| {
            exe_path.parent()
                .and_then(|p| p.parent())
                .map(|pkg_root| pkg_root.join("share/voicevox/models"))
        });
    
    let workspace_path = std::env::current_dir()
        .ok()
        .and_then(|current_dir| {
            current_dir
                .ancestors()
                .find(|a| a.join("models").exists())
                .map(|p| p.join("models"))
        });
    
    // Compose all paths using functional chain
    env_based_paths
        .chain(static_based_paths)
        .chain(package_path)
        .chain(workspace_path)
        .collect()
}

// Helper function to check if a directory contains .dic files (including subdirectories)
#[allow(dead_code)]
fn has_dic_files(dict_path: &PathBuf) -> bool {
    has_direct_dic_files(dict_path) || find_dic_subdir(dict_path).is_some()
}

// Helper function to check if a directory contains .dic files directly
fn has_direct_dic_files(dict_path: &PathBuf) -> bool {
    let entries = match std::fs::read_dir(dict_path) {
        Ok(entries) => entries,
        Err(_) => return false,
    };
    
    entries.filter_map(|e| e.ok()).any(|entry| {
        entry.file_name().to_str()
            .map_or(false, |name| name.ends_with(".dic"))
    })
}

// Helper function to find subdirectory containing .dic files (for VOICEVOX downloader structure)
fn find_dic_subdir(dict_path: &PathBuf) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dict_path).ok()?;
    
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        
        if has_direct_dic_files(&path) {
            return Some(path);
        }
    }
    
    None
}

pub fn find_openjtalk_dict() -> Result<String> {
    let mut search_paths = Vec::new();

    // Environment variables with their corresponding paths
    let env_paths = [
        ("XDG_DATA_HOME", "voicevox/dict"),
        ("HOME", ".local/share/voicevox/dict"),
        ("HOME", ".voicevox/dict"), // Legacy
        ("VOICEVOX_DICT_DIR", ""),
        ("OPENJTALK_DICT_DIR", ""),
    ];
    
    // Static system paths
    let static_paths = [
        "./voicevox_core/dict",
        "./dict",
        "/usr/local/share/open-jtalk/dic",
        "/usr/share/open-jtalk/dic", 
        "/opt/open-jtalk/dic",
        "/usr/local/share/voicevox/dict",
        "/usr/share/voicevox/dict",
        "/opt/voicevox/dict",
        "/Applications/VOICEVOX.app/Contents/Resources/dict",
        "/opt/homebrew/share/open-jtalk/dic",
        "/opt/homebrew/share/voicevox/dict",
        "/opt/local/share/open-jtalk/dic",
    ];
    
    // Build paths using functional composition
    let env_based_paths = env_paths
        .iter()
        .filter_map(|(env_var, suffix)| {
            std::env::var(env_var).ok().map(|env_value| {
                if suffix.is_empty() {
                    PathBuf::from(env_value)
                } else {
                    PathBuf::from(env_value).join(suffix)
                }
            })
        });
    
    let static_based_paths = static_paths
        .iter()
        .map(|p| PathBuf::from(p));
    
    let package_path = std::env::current_exe()
        .ok()
        .and_then(|exe_path| {
            exe_path.parent()
                .and_then(|p| p.parent())
                .map(|pkg_root| pkg_root.join("share/voicevox/dict"))
        });
    
    let workspace_path = std::env::current_dir()
        .ok()
        .and_then(|current_dir| {
            current_dir
                .ancestors()
                .find(|a| a.join("dict").exists())
                .map(|p| p.join("dict"))
        });
    
    // Compose all paths using functional chain
    search_paths.extend(
        env_based_paths
            .chain(static_based_paths)
            .chain(package_path)
            .chain(workspace_path)
    );

    search_paths
        .into_iter()
        .filter(|path| path.exists())
        .find_map(|path| {
            // First check if dictionary files are directly in this directory
            if has_direct_dic_files(&path) {
                Some(path.to_string_lossy().to_string())
            } else {
                // Then check subdirectories for dictionary files
                find_dic_subdir(&path)
                    .map(|dict_subdir| dict_subdir.to_string_lossy().to_string())
            }
        })
        .ok_or_else(|| {
            anyhow!("OpenJTalk dictionary not found. Please ensure the dictionary is installed in one of the standard locations or set VOICEVOX_DICT_DIR/OPENJTALK_DICT_DIR environment variable.")
        })
}