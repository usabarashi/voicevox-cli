use std::path::PathBuf;

#[must_use]
pub fn load_mcp_instructions() -> Option<String> {
    if let Ok(inline) = std::env::var(crate::config::ENV_VOICEVOX_MCP_INSTRUCTIONS) {
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

    if let Some(config_home) = std::env::var_os(crate::config::ENV_XDG_CONFIG_HOME) {
        candidates.push(
            PathBuf::from(config_home)
                .join(crate::config::APP_NAME)
                .join(crate::config::MCP_INSTRUCTIONS_FILE),
        );
    } else if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join(crate::config::USER_CONFIG_DIR)
                .join(crate::config::APP_NAME)
                .join(crate::config::MCP_INSTRUCTIONS_FILE),
        );
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join(crate::config::MCP_INSTRUCTIONS_FILE));
        }
    }

    candidates.push(PathBuf::from(crate::config::MCP_INSTRUCTIONS_FILE));
    candidates
}
