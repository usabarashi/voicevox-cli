use std::path::PathBuf;

const INSTRUCTIONS_ENV: &str = "VOICEVOX_MCP_INSTRUCTIONS";
const INSTRUCTIONS_FILE: &str = "VOICEVOX.md";
const APP_NAME: &str = "voicevox";

#[must_use]
pub fn load_mcp_instructions() -> Option<String> {
    if let Ok(inline) = std::env::var(INSTRUCTIONS_ENV) {
        let trimmed = inline.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    instruction_candidates()
        .into_iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .map(|content| content.trim().to_string())
        .filter(|content| !content.is_empty())
}

fn instruction_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        candidates.push(
            PathBuf::from(config_home)
                .join(APP_NAME)
                .join(INSTRUCTIONS_FILE),
        );
    } else if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".config").join(APP_NAME).join(INSTRUCTIONS_FILE));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(INSTRUCTIONS_FILE));
        }
    }

    candidates.push(PathBuf::from(INSTRUCTIONS_FILE));
    candidates
}
