use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

use crate::infrastructure::daemon::client::DaemonClient;
use crate::infrastructure::voicevox::{AvailableModel, Speaker, scan_available_models};
use crate::interface::synthesis::flow::connect_daemon_client_auto_start;
use crate::interface::{AppOutput, StdAppOutput};

const NO_MODELS_MESSAGE: &str =
    "No voice models found. Please run 'voicevox-setup' to download required resources.";

fn format_speaker_block(speaker: &Speaker, style_to_model: Option<&HashMap<u32, u32>>) -> String {
    let style_lines = speaker
        .styles
        .iter()
        .flat_map(|style| {
            let main_line = match style_to_model.and_then(|map| map.get(&style.id)) {
                Some(model_id) => format!(
                    "    {} (Model: {model_id}, Style ID: {})",
                    style.name, style.id
                ),
                None => format!("    {} (Style ID: {})", style.name, style.id),
            };

            std::iter::once(main_line).chain(
                style
                    .style_type
                    .iter()
                    .map(|style_type| format!("        Type: {style_type}")),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("  {}\n{style_lines}", speaker.name)
}

fn format_speakers_output(
    header: &str,
    speakers: &[Speaker],
    style_to_model: Option<&HashMap<u32, u32>>,
) -> String {
    let body = speakers
        .iter()
        .map(|speaker| format_speaker_block(speaker, style_to_model))
        .collect::<Vec<_>>()
        .join("\n\n");

    if body.is_empty() {
        header.to_string()
    } else {
        format!("{header}\n{body}\n")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelView {
    model_id: u32,
    file_path: String,
    default_style_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledModelView {
    model_id: u32,
    file_path: String,
    file_name: Option<String>,
    size_kb: Option<u64>,
}

fn missing_status_lines(name: &str) -> [String; 2] {
    [
        format!("{name}: Not found"),
        "  Install with: voicevox-setup".to_string(),
    ]
}

fn list_models_lines(models: &[ModelView]) -> Vec<String> {
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

fn status_models_lines(models: &[InstalledModelView]) -> Vec<String> {
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

fn print_no_models_message(output: &dyn AppOutput) {
    output.info(NO_MODELS_MESSAGE);
}

fn handle_missing_models_error(error: anyhow::Error, output: &dyn AppOutput) -> Result<()> {
    if crate::infrastructure::paths::find_models_dir().is_err() {
        print_no_models_message(output);
        return Ok(());
    }

    Err(error)
}

fn print_list_models_output(models: &[AvailableModel], output: &dyn AppOutput) {
    let views = models
        .iter()
        .map(|model| ModelView {
            model_id: model.model_id,
            file_path: model.file_path.display().to_string(),
            default_style_id: model
                .speakers
                .iter()
                .flat_map(|speaker| speaker.styles.iter())
                .map(|style| style.id)
                .min(),
        })
        .collect::<Vec<_>>();
    for line in list_models_lines(&views) {
        output.info(&line);
    }
}

pub async fn run_list_models_command(socket_path: &Path) -> Result<()> {
    let output = StdAppOutput;
    run_list_models_command_with_output(socket_path, &output).await
}

pub async fn run_list_models_command_with_output(
    socket_path: &Path,
    output: &dyn AppOutput,
) -> Result<()> {
    match connect_daemon_client_auto_start(socket_path).await {
        Ok(mut client) => {
            let models = client.list_models().await?;
            print_list_models_output(&models, output);
            Ok(())
        }
        Err(error) => handle_missing_models_error(error, output),
    }
}

fn print_status_models(current_models: &[AvailableModel], output: &dyn AppOutput) {
    let views = current_models
        .iter()
        .map(|model| {
            let metadata = std::fs::metadata(&model.file_path);
            let size_kb = metadata.as_ref().ok().map(|value| value.len() / 1024);
            let file_name = model
                .file_path
                .file_name()
                .map(|value| value.to_string_lossy().into_owned());
            InstalledModelView {
                model_id: model.model_id,
                file_path: model.file_path.display().to_string(),
                file_name,
                size_kb,
            }
        })
        .collect::<Vec<_>>();
    for line in status_models_lines(&views) {
        output.info(&line);
    }
}

fn print_missing_status_item(name: &str, output: &dyn AppOutput) {
    for line in missing_status_lines(name) {
        output.info(&line);
    }
}

fn print_status_dictionary(output: &dyn AppOutput) {
    match crate::infrastructure::paths::find_openjtalk_dict() {
        Ok(dict_path) => {
            output.info(&format!("Dictionary: {}", dict_path.display()));
        }
        _ => {
            print_missing_status_item("Dictionary", output);
        }
    }
}

pub fn run_status_command() {
    let output = StdAppOutput;
    run_status_command_with_output(&output);
}

pub fn run_status_command_with_output(output: &dyn AppOutput) {
    output.info("VOICEVOX CLI Installation Status");
    output.info("=====================================");
    output.info(&format!("Application: v{}", env!("CARGO_PKG_VERSION")));

    match crate::infrastructure::paths::find_onnxruntime() {
        Ok(onnx_path) => {
            output.info(&format!("ONNX Runtime: {}", onnx_path.display()));
        }
        _ => {
            print_missing_status_item("ONNX Runtime", output);
        }
    }

    match scan_available_models() {
        Ok(current_models) => {
            print_status_models(&current_models, output);
            print_status_dictionary(output);
        }
        Err(error) => {
            print_missing_status_item("Voice Models", output);
            output.error(&format!("Error scanning models: {error}"));
        }
    }
}

fn print_speakers(speakers: &[Speaker], output: &dyn AppOutput) {
    output.info(&format_speakers_output(
        "All available speakers and styles:",
        speakers,
        None,
    ));
}

pub async fn run_list_speakers_command(socket_path: &Path) -> Result<()> {
    let output = StdAppOutput;
    run_list_speakers_command_with_output(socket_path, &output).await
}

pub async fn run_list_speakers_command_with_output(
    socket_path: &Path,
    output: &dyn AppOutput,
) -> Result<()> {
    if let Ok(mut client) = DaemonClient::new_at(socket_path).await {
        let (speakers, style_to_model) = client.list_speakers_with_models().await?;
        output.info(&format_speakers_output(
            "All available speakers and styles from daemon:",
            &speakers,
            Some(&style_to_model),
        ));
        return Ok(());
    }

    match connect_daemon_client_auto_start(socket_path).await {
        Ok(mut client) => {
            let speakers = client.list_speakers().await?;
            print_speakers(&speakers, output);
            Ok(())
        }
        Err(error) => handle_missing_models_error(error, output),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::voicevox::{Speaker, Style};
    use crate::interface::output::BufferAppOutput;
    use std::path::PathBuf;

    #[test]
    fn print_list_models_output_shows_no_models_message() {
        let output = BufferAppOutput::default();

        print_list_models_output(&[], &output);

        assert_eq!(output.infos(), vec![NO_MODELS_MESSAGE.to_string()]);
    }

    #[test]
    fn print_list_models_output_includes_default_style_and_tips() {
        let output = BufferAppOutput::default();
        let models = vec![AvailableModel {
            model_id: 12,
            file_path: PathBuf::from("/tmp/12.vvm"),
            speakers: vec![Speaker {
                name: "Test Speaker".into(),
                speaker_uuid: String::new().into(),
                styles: vec![
                    Style {
                        name: "Normal".into(),
                        id: 42,
                        style_type: None,
                    },
                    Style {
                        name: "Happy".into(),
                        id: 7,
                        style_type: Some("talk".into()),
                    },
                ]
                .into(),
                version: String::new().into(),
            }]
            .into(),
        }];

        print_list_models_output(&models, &output);

        let infos = output.infos().join("\n");
        assert!(infos.contains("Available voice models:"));
        assert!(infos.contains("Model 12 (/tmp/12.vvm)"));
        assert!(infos.contains("Default style ID (auto-selected by --model): 7"));
        assert!(infos.contains("Use --list-speakers for detailed speaker information"));
    }
}
