use anyhow::{anyhow, Result};
use std::path::PathBuf;

// Socket path for IPC - user-specific for daemon isolation
pub fn get_socket_path() -> PathBuf {
    // Priority 1: Environment variable override
    if let Ok(custom_path) = std::env::var("VOICEVOX_SOCKET_PATH") {
        return PathBuf::from(custom_path);
    }

    // Priority 2: XDG_RUNTIME_DIR (user-specific)
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let socket_path = PathBuf::from(runtime_dir).join("voicevox-daemon.sock");
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return socket_path;
    }

    // Priority 3: XDG_STATE_HOME (user-specific persistent)
    if let Ok(state_dir) = std::env::var("XDG_STATE_HOME") {
        let socket_path = PathBuf::from(state_dir).join("voicevox-daemon.sock");
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return socket_path;
    }

    // Priority 4: User home directory fallback
    if let Ok(home_dir) = std::env::var("HOME") {
        let socket_path = PathBuf::from(home_dir).join(".local/state/voicevox-daemon.sock");
        if let Some(parent) = socket_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return socket_path;
    }

    // Priority 5: User-specific temp socket with UID (not PID)
    let user_id = unsafe { libc::getuid() };
    PathBuf::from("/tmp").join(format!("voicevox-daemon-{}.sock", user_id))
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
    let mut search_paths = Vec::new();

    // Priority 1: Environment variable override (admin/CI systems)
    if let Some(env_path) = std::env::var("VOICEVOX_MODELS_DIR").ok() {
        search_paths.push(PathBuf::from(env_path));
    }

    // Priority 2: XDG compliant user directory (VOICEVOX downloader standard)
    if let Ok(home_dir) = std::env::var("HOME") {
        search_paths.push(PathBuf::from(home_dir).join(".local/share/voicevox/models/vvms"));
    }

    // Priority 3: Local VOICEVOX core directory (downloaded by downloader)
    search_paths.push(PathBuf::from("./voicevox_core/models"));

    // Priority 4: Package installation path (when used as a Nix package)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(pkg_root) = exe_path.parent().and_then(|p| p.parent()) {
            search_paths.push(pkg_root.join("share/voicevox/models"));
        }
    }

    // Priority 5: System shared directories (fallback only)
    search_paths.extend([
        PathBuf::from("/usr/local/share/voicevox/models"),
        PathBuf::from("/usr/share/voicevox/models"),
        PathBuf::from("/opt/voicevox/models"),
        PathBuf::from("/opt/homebrew/share/voicevox/models"), // macOS Homebrew
    ]);

    // Priority 6: macOS application bundle
    search_paths.push(PathBuf::from(
        "/Applications/VOICEVOX.app/Contents/Resources/models",
    ));

    let additional_paths = vec![
        // Priority 7: Current working directory (development)
        Some(PathBuf::from("./models")),
        Some(PathBuf::from("./voicevox_models/models/vvms")), // Nix development layout
        // Priority 8: User-specific directories (fallback only)
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(|xdg| PathBuf::from(xdg).join("voicevox/models"))
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".local/share/voicevox/models"))
            }),
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".voicevox/models")),
        // Priority 7: Development/workspace paths (generic search)
        std::env::current_dir().ok().and_then(|current_dir| {
            current_dir
                .ancestors()
                .find(|a| a.join("models").exists())
                .map(|p| p.join("models"))
        }),
    ];

    search_paths.extend(additional_paths.into_iter().flatten());
    search_paths
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

    let additional_paths = vec![
        // Priority 1: VOICEVOX downloader standard locations (XDG compliant)
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(|xdg| PathBuf::from(xdg).join("voicevox/dict"))
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".local/share/voicevox/dict"))
            }),
        // Priority 2: Local VOICEVOX core directory (downloaded by downloader)
        Some(PathBuf::from("./voicevox_core/dict")),
        Some(PathBuf::from("./dict")),
        // Priority 3: Package installation path (when used as a Nix package)
        std::env::current_exe()
            .ok()
            .and_then(|exe_path| {
                exe_path.parent()
                    .and_then(|p| p.parent())
                    .map(|pkg_root| pkg_root.to_path_buf())
            })
            .map(|pkg_root| pkg_root.join("share/voicevox/dict")),
        // Priority 4: Legacy home directory dictionary
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".voicevox/dict")),
        // Priority 5: System OpenJTalk paths
        Some(PathBuf::from("/usr/local/share/open-jtalk/dic")),
        Some(PathBuf::from("/usr/share/open-jtalk/dic")),
        Some(PathBuf::from("/opt/open-jtalk/dic")),
        // Priority 6: System VOICEVOX paths
        Some(PathBuf::from("/usr/local/share/voicevox/dict")),
        Some(PathBuf::from("/usr/share/voicevox/dict")),
        Some(PathBuf::from("/opt/voicevox/dict")),
        // Priority 7: macOS specific paths
        Some(PathBuf::from(
            "/Applications/VOICEVOX.app/Contents/Resources/dict",
        )),
        Some(PathBuf::from("/opt/homebrew/share/open-jtalk/dic")),
        Some(PathBuf::from("/opt/homebrew/share/voicevox/dict")),
        Some(PathBuf::from("/opt/local/share/open-jtalk/dic")),
        // Priority 8: Development/workspace paths (generic search)
        std::env::current_dir().ok().and_then(|current_dir| {
            current_dir
                .ancestors()
                .find(|a| a.join("dict").exists())
                .map(|p| p.join("dict"))
        }),
        // Priority 9: Environment variable (explicit override)
        std::env::var("VOICEVOX_DICT_DIR").ok().map(PathBuf::from),
        std::env::var("OPENJTALK_DICT_DIR").ok().map(PathBuf::from),
    ];

    search_paths.extend(additional_paths.into_iter().flatten());

    for path_option in search_paths.into_iter() {
        if !path_option.exists() {
            continue;
        }
        
        // First check if dictionary files are directly in this directory
        if has_direct_dic_files(&path_option) {
            return Ok(path_option.to_string_lossy().to_string());
        }
        
        // Then check subdirectories for dictionary files
        if let Some(dict_subdir) = find_dic_subdir(&path_option) {
            return Ok(dict_subdir.to_string_lossy().to_string());
        }
    }

    Err(anyhow!("OpenJTalk dictionary not found. Please ensure the dictionary is installed in one of the standard locations or set VOICEVOX_DICT_DIR/OPENJTALK_DICT_DIR environment variable."))
}