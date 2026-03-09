use anyhow::{Result, anyhow};

use crate::infrastructure::voicevox::scan_available_models;

/// Resolves CLI voice input into a style/model ID and description.
///
/// # Errors
///
/// Returns an error if model discovery fails or the input cannot be resolved.
pub fn resolve_voice_input(voice_input: &str) -> Result<(u32, String)> {
    let voice_input = voice_input.trim();

    if voice_input == "?" {
        return Err(anyhow!("Voice help is a CLI concern."));
    }

    voice_input
        .parse::<u32>()
        .ok()
        .filter(|&id| id > 0 && id < 1000)
        .map(|style_id| (style_id, format!("Style ID {style_id}")))
        .map_or_else(|| try_resolve_from_available_models(voice_input), Ok)
}

fn try_resolve_from_available_models(voice_input: &str) -> Result<(u32, String)> {
    let available_models = scan_available_models().map_err(|e| {
        anyhow!(
            "Failed to scan available models: {e}. Use --speaker-id for direct ID specification."
        )
    })?;

    if available_models.is_empty() {
        return Err(anyhow!(
            "No voice models available. Please download models first or use --speaker-id for direct ID specification."
        ));
    }

    voice_input
        .parse::<u32>()
        .ok()
        .filter(|&model_id| available_models.iter().any(|m| m.model_id == model_id))
        .map(|model_id| (model_id, format!("Model {model_id} (Default Style)")))
        .map_or_else(
            || {
                let model_suggestions = available_models
                    .iter()
                    .take(3)
                    .map(|m| format!("--model {}", m.model_id))
                    .collect::<Vec<_>>()
                    .join(", ");

                Err(anyhow!(
                    "Voice '{voice_input}' not found. Available options:\n  \
                    Use --speaker-id N for direct style ID\n  \
                    Use --model N for model selection (e.g., {model_suggestions})\n  \
                    Use --list-models to see all {} available models\n  \
                    Use --list-speakers for detailed speaker information",
                    available_models.len()
                ))
            },
            Ok,
        )
}

#[cfg(test)]
mod tests {
    use super::resolve_voice_input;

    #[test]
    fn resolve_voice_input_trims_direct_style_id() {
        let (style_id, description) =
            resolve_voice_input("  3  ").expect("trimmed numeric style id should resolve");
        assert_eq!(style_id, 3);
        assert_eq!(description, "Style ID 3");
    }
}
