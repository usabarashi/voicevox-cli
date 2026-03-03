pub const NO_MODELS_MESSAGE: &str =
    "No voice models found. Please run 'voicevox-setup' to download required resources.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelView {
    pub model_id: u32,
    pub file_path: String,
    pub default_style_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModelView {
    pub model_id: u32,
    pub file_path: String,
    pub file_name: Option<String>,
    pub size_kb: Option<u64>,
}

#[must_use]
pub fn missing_status_lines(name: &str) -> [String; 2] {
    [
        format!("{name}: Not found"),
        "  Install with: voicevox-setup".to_string(),
    ]
}

#[must_use]
pub fn list_models_lines(models: &[ModelView]) -> Vec<String> {
    if models.is_empty() {
        return vec![NO_MODELS_MESSAGE.to_string()];
    }

    let mut lines = vec!["Available voice models:".to_string()];
    for model in models {
        lines.push(format!("  Model {} ({})", model.model_id, model.file_path));
        lines.push(format!(
            "    Usage: --model {} or --speaker-id <STYLE_ID>",
            model.model_id
        ));
        if let Some(default_style_id) = model.default_style_id {
            lines.push(format!(
                "    Default style ID (auto-selected by --model): {default_style_id}"
            ));
        }
    }
    lines.push("\nTips:".to_string());
    lines.push("  - Use --model N to load model N.vvm".to_string());
    lines.push("  - Use --speaker-id for direct style ID specification".to_string());
    lines.push("  - Use --list-speakers for detailed speaker information".to_string());
    lines
}

#[must_use]
pub fn status_models_lines(models: &[InstalledModelView]) -> Vec<String> {
    if models.is_empty() {
        return missing_status_lines("Voice Models").into();
    }

    let mut lines = vec![format!("Voice Models: {} files installed", models.len())];
    for model in models {
        let line = match (&model.file_name, model.size_kb) {
            (Some(name), Some(size_kb)) => {
                format!("  Model {}: {name} ({size_kb} KB)", model.model_id)
            }
            _ => format!("  Model {} ({})", model.model_id, model.file_path),
        };
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{list_models_lines, status_models_lines, ModelView};

    #[test]
    fn list_models_lines_returns_no_models_message_when_empty() {
        let lines = list_models_lines(&[]);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("No voice models found"));
    }

    #[test]
    fn list_models_lines_contains_default_style() {
        let lines = list_models_lines(&[ModelView {
            model_id: 12,
            file_path: "/tmp/12.vvm".to_string(),
            default_style_id: Some(7),
        }]);
        let joined = lines.join("\n");
        assert!(joined.contains("Model 12 (/tmp/12.vvm)"));
        assert!(joined.contains("Default style ID (auto-selected by --model): 7"));
    }

    #[test]
    fn status_models_lines_with_metadata() {
        let lines = status_models_lines(&[super::InstalledModelView {
            model_id: 1,
            file_path: "/tmp/1.vvm".to_string(),
            file_name: Some("1.vvm".to_string()),
            size_kb: Some(123),
        }]);
        let joined = lines.join("\n");
        assert!(joined.contains("Voice Models: 1 files installed"));
        assert!(joined.contains("Model 1: 1.vvm (123 KB)"));
    }
}
