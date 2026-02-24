use anyhow::Result;
use std::path::Path;

use crate::app::{AppOutput, StdAppOutput};
use crate::client::{list_speakers_daemon, DaemonClient};
use crate::paths::find_openjtalk_dict;
use crate::voice::{format_speakers_output, scan_available_models, AvailableModel, Speaker};

const NO_MODELS_MESSAGE: &str =
    "No voice models found. Please run 'voicevox-setup' to download required resources.";

fn print_no_models_message(output: &dyn AppOutput) {
    output.info(NO_MODELS_MESSAGE);
}

fn handle_missing_models_error(error: anyhow::Error, output: &dyn AppOutput) -> Result<()> {
    if crate::paths::find_models_dir().is_err() {
        print_no_models_message(output);
        return Ok(());
    }

    Err(error)
}

fn print_list_models_output(models: &[AvailableModel], output: &dyn AppOutput) {
    if models.is_empty() {
        print_no_models_message(output);
        return;
    }

    output.info("Available voice models:");
    for model in models {
        output.info(&format!(
            "  Model {} ({})",
            model.model_id,
            model.file_path.display()
        ));
        output.info(&format!(
            "    Usage: --model {} or --speaker-id <STYLE_ID>",
            model.model_id
        ));
        if let Some(default_style_id) = model
            .speakers
            .iter()
            .flat_map(|speaker| speaker.styles.iter())
            .map(|style| style.id)
            .min()
        {
            output.info(&format!(
                "    Default style ID (auto-selected by --model): {default_style_id}"
            ));
        }
    }

    output.info("\nTips:");
    output.info("  - Use --model N to load model N.vvm");
    output.info("  - Use --speaker-id for direct style ID specification");
    output.info("  - Use --list-speakers for detailed speaker information");
}

pub async fn run_list_models_command(socket_path: &Path) -> Result<()> {
    let output = StdAppOutput;
    run_list_models_command_with_output(socket_path, &output).await
}

pub async fn run_list_models_command_with_output(
    socket_path: &Path,
    output: &dyn AppOutput,
) -> Result<()> {
    match DaemonClient::new_with_auto_start_at(socket_path).await {
        Ok(mut client) => {
            let models = client.list_models().await?;
            print_list_models_output(&models, output);
            Ok(())
        }
        Err(error) => handle_missing_models_error(error, output),
    }
}

fn print_status_models(current_models: &[AvailableModel], output: &dyn AppOutput) {
    if current_models.is_empty() {
        print_missing_status_item("Voice Models", output);
        return;
    }

    output.info(&format!(
        "Voice Models: {} files installed",
        current_models.len()
    ));
    for model in current_models {
        let model_info = std::fs::metadata(&model.file_path).map_or_else(
            |_| format!("  Model {} ({})", model.model_id, model.file_path.display()),
            |metadata| {
                let size_kb = metadata.len() / 1024;
                let filename = model
                    .file_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                format!("  Model {}: {filename} ({size_kb} KB)", model.model_id)
            },
        );
        output.info(&model_info);
    }
}

fn print_missing_status_item(name: &str, output: &dyn AppOutput) {
    output.info(&format!("{name}: Not found"));
    output.info("  Install with: voicevox-setup");
}

fn print_status_dictionary(output: &dyn AppOutput) {
    if let Ok(dict_path) = find_openjtalk_dict() {
        output.info(&format!("Dictionary: {}", dict_path.display()));
    } else {
        print_missing_status_item("Dictionary", output);
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

    if let Ok(onnx_path) = crate::paths::find_onnxruntime() {
        output.info(&format!("ONNX Runtime: {}", onnx_path.display()));
    } else {
        print_missing_status_item("ONNX Runtime", output);
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
    if list_speakers_daemon(socket_path).await.is_ok() {
        return Ok(());
    }

    match DaemonClient::new_with_auto_start_at(socket_path).await {
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
    use crate::app::output::BufferAppOutput;
    use crate::voice::{Speaker, Style};
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
