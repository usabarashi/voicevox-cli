use serde::{Deserialize, Serialize};

pub const APP_NAME: &str = "voicevox";
pub const SOCKET_FILENAME: &str = "voicevox-daemon.sock";
pub const MCP_INSTRUCTIONS_FILE: &str = "VOICEVOX.md";

pub const ENV_HOME: &str = "HOME";
pub const ENV_PATH: &str = "PATH";
pub const ENV_XDG_CONFIG_HOME: &str = "XDG_CONFIG_HOME";
pub const ENV_XDG_DATA_HOME: &str = "XDG_DATA_HOME";
pub const ENV_XDG_RUNTIME_DIR: &str = "XDG_RUNTIME_DIR";
pub const ENV_XDG_STATE_HOME: &str = "XDG_STATE_HOME";
pub const ENV_ORT_DYLIB_PATH: &str = "ORT_DYLIB_PATH";

pub const ENV_VOICEVOX_SOCKET_PATH: &str = "VOICEVOX_SOCKET_PATH";
pub const ENV_VOICEVOX_MODELS_DIR: &str = "VOICEVOX_MODELS_DIR";
pub const ENV_VOICEVOX_OPENJTALK_DICT: &str = "VOICEVOX_OPENJTALK_DICT";
pub const ENV_VOICEVOX_MCP_INSTRUCTIONS: &str = "VOICEVOX_MCP_INSTRUCTIONS";
pub const ENV_VOICEVOX_LOW_LATENCY: &str = "VOICEVOX_LOW_LATENCY";
pub const ENV_VOICEVOX_DETACH_PARENT_PID: &str = "VOICEVOX_DETACH_PARENT_PID";
pub const ENV_VOICEVOX_ALLOW_UNSAFE_PATH_COMMANDS: &str = "VOICEVOX_ALLOW_UNSAFE_PATH_COMMANDS";
pub const ENV_VOICEVOX_ALLOW_UNSAFE_DAEMON_LOOKUP: &str = "VOICEVOX_ALLOW_UNSAFE_DAEMON_LOOKUP";

pub const DEFAULT_TMP_DIR: &str = "/tmp";
pub const USER_CONFIG_DIR: &str = ".config";
pub const USER_LOCAL_SHARE_DIR: &str = ".local/share";
pub const USER_LOCAL_STATE_DIR: &str = ".local/state";

pub const SYSTEM_PGREP_PATH: &str = "/usr/bin/pgrep";
pub const SYSTEM_PS_PATH: &str = "/bin/ps";
pub const SYSTEM_KILL_PATH: &str = "/bin/kill";

pub const SYSTEM_AUDIO_PLAYER_PATHS: [&str; 3] = [
    "/usr/bin/afplay",
    "/opt/homebrew/bin/play",
    "/usr/local/bin/play",
];
pub const FALLBACK_AUDIO_PLAYERS: [&str; 2] = ["afplay", "play"];

pub const SYSTEM_VOICEVOX_LIB_DIRS: [&str; 2] =
    ["/usr/local/share/voicevox/lib", "/opt/voicevox/lib"];

#[must_use]
pub fn allow_unsafe_path_commands() -> bool {
    std::env::var_os(ENV_VOICEVOX_ALLOW_UNSAFE_PATH_COMMANDS).is_some()
}

#[must_use]
pub fn command_path_or_fallback(
    preferred: &'static str,
    fallback_name: &'static str,
) -> &'static str {
    if std::path::Path::new(preferred).is_file() {
        preferred
    } else if allow_unsafe_path_commands() {
        fallback_name
    } else {
        preferred
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub text_splitter: TextSplitterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSplitterConfig {
    #[serde(default = "default_delimiters")]
    pub delimiters: Vec<String>,
    #[serde(default = "default_max_length")]
    pub max_length: usize,
}

impl Default for TextSplitterConfig {
    fn default() -> Self {
        Self {
            delimiters: default_delimiters(),
            max_length: default_max_length(),
        }
    }
}

fn default_delimiters() -> Vec<String> {
    vec![
        "。".to_string(),
        "！".to_string(),
        "？".to_string(),
        "．".to_string(),
        "\n".to_string(),
    ]
}

const fn default_max_length() -> usize {
    100
}
