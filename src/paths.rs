use anyhow::{anyhow, Result};
use std::path::PathBuf;

const APP_NAME: &str = "voicevox";
const MODELS_SUBDIR: &str = "models";
const OPENJTALK_DICT_SUBDIR: &str = "openjtalk_dict";
const SOCKET_FILENAME: &str = "voicevox-daemon.sock";

// XDG Base Directory fallback paths
const LOCAL_SHARE: &str = ".local/share";
const LOCAL_STATE: &str = ".local/state";

/// Get the default VOICEVOX data directory path following XDG Base Directory specification
///
/// Priority:
/// 1. $XDG_DATA_HOME/voicevox (default: ~/.local/share/voicevox)
/// 2. ~/.local/share/voicevox (fallback)
pub fn get_default_voicevox_dir() -> PathBuf {
    dirs::data_local_dir()
        .map(|d| d.join(APP_NAME))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(LOCAL_SHARE).join(APP_NAME))
                .unwrap_or_else(|| PathBuf::from("."))
        })
}

/// Get the default models directory path
pub fn get_default_models_dir() -> PathBuf {
    get_default_voicevox_dir().join(MODELS_SUBDIR)
}

/// Get socket path following XDG Base Directory specification
///
/// Priority:
/// 1. $VOICEVOX_SOCKET_PATH (explicit override)
/// 2. $XDG_RUNTIME_DIR/voicevox-daemon.sock (runtime files)
/// 3. $XDG_STATE_HOME/voicevox-daemon.sock (persistent state)
/// 4. ~/.local/state/voicevox-daemon.sock (fallback)
/// 5. /tmp/voicevox-daemon.sock (last resort)
pub fn get_socket_path() -> PathBuf {
    // Check explicit override first
    if let Ok(path) = std::env::var("VOICEVOX_SOCKET_PATH") {
        return PathBuf::from(path);
    }

    // XDG_RUNTIME_DIR is preferred for runtime files
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join(SOCKET_FILENAME);
    }

    // XDG_STATE_HOME for persistent state
    if let Some(state_dir) = dirs::state_dir() {
        return state_dir.join(SOCKET_FILENAME);
    }

    // Fallback to ~/.local/state if available
    if let Some(home) = dirs::home_dir() {
        let local_state = home.join(LOCAL_STATE);
        if local_state.exists() || std::fs::create_dir_all(&local_state).is_ok() {
            return local_state.join(SOCKET_FILENAME);
        }
    }

    // Last resort: /tmp
    PathBuf::from("/tmp").join(SOCKET_FILENAME)
}

/// Find models directory following XDG Base Directory specification
///
/// Priority:
/// 1. $VOICEVOX_MODELS_DIR (explicit override)
/// 2. $XDG_DATA_HOME/voicevox/models (user data)
/// 3. ~/.local/share/voicevox/models (fallback)
pub fn find_models_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("VOICEVOX_MODELS_DIR") {
        let models_dir = PathBuf::from(path);
        if models_dir.exists() && models_dir.is_dir() {
            return Ok(models_dir);
        }
    }

    // XDG-compliant user data directory
    if let Some(data_dir) = dirs::data_local_dir() {
        let user_models_path = data_dir.join(APP_NAME).join(MODELS_SUBDIR);
        if user_models_path.exists() && user_models_path.is_dir() {
            return Ok(user_models_path);
        }
    }

    // Fallback to ~/.local/share/voicevox
    if let Some(home) = dirs::home_dir() {
        let fallback_path = home.join(LOCAL_SHARE).join(APP_NAME).join(MODELS_SUBDIR);
        if fallback_path.exists() && fallback_path.is_dir() {
            return Ok(fallback_path);
        }
    }

    Err(anyhow!(
        "Models directory not found. Please set $VOICEVOX_MODELS_DIR or place models in $XDG_DATA_HOME/{}/{} (default: ~/.local/share/{}/{})",
        APP_NAME,
        MODELS_SUBDIR,
        APP_NAME,
        MODELS_SUBDIR
    ))
}

/// Find models directory for client operations (with XDG compliance)
pub fn find_models_dir_client() -> Result<PathBuf> {
    match find_models_dir() {
        Ok(dir) => Ok(dir),
        Err(_) => {
            // XDG-compliant default path
            let default_path = dirs::data_local_dir()
                .map(|d| d.join(APP_NAME))
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .map(|h| h.join(LOCAL_SHARE).join(APP_NAME))
                        .unwrap_or_else(|| PathBuf::from("."))
                })
                .join(MODELS_SUBDIR);

            // Check parent directory as alternative
            let alternative_path = default_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(get_default_voicevox_dir);

            if alternative_path.exists() && alternative_path.is_dir() {
                Ok(alternative_path)
            } else {
                Ok(default_path)
            }
        }
    }
}

/// Find OpenJTalk dictionary following XDG Base Directory specification
///
/// Priority:
/// 1. $VOICEVOX_OPENJTALK_DICT (explicit override)
/// 2. $XDG_DATA_HOME/voicevox/openjtalk_dict (user data)
/// 3. ~/.local/share/voicevox/openjtalk_dict (fallback)
pub fn find_openjtalk_dict() -> Result<PathBuf> {
    // Check explicit override first
    if let Ok(path) = std::env::var("VOICEVOX_OPENJTALK_DICT") {
        let dict_path = PathBuf::from(path);
        if dict_path.exists() && dict_path.is_dir() {
            return Ok(dict_path);
        }
    }

    // XDG-compliant user data directory
    if let Some(data_dir) = dirs::data_local_dir() {
        let user_dict_path = data_dir.join(APP_NAME).join(OPENJTALK_DICT_SUBDIR);
        if user_dict_path.exists() && user_dict_path.is_dir() {
            return Ok(user_dict_path);
        }
    }

    // Fallback to ~/.local/share/voicevox
    if let Some(home) = dirs::home_dir() {
        let fallback_path = home
            .join(LOCAL_SHARE)
            .join(APP_NAME)
            .join(OPENJTALK_DICT_SUBDIR);
        if fallback_path.exists() && fallback_path.is_dir() {
            return Ok(fallback_path);
        }
    }

    Err(anyhow!(
        "OpenJTalk dictionary not found. It will be downloaded automatically on first use. \
         Run 'voicevox-say' to trigger the setup process."
    ))
}
