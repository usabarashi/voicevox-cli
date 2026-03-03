use anyhow::Result;
use std::path::Path;

use crate::domain::inspect::{
    list_models_lines, missing_status_lines, status_models_lines, InstalledModelView, ModelView,
    NO_MODELS_MESSAGE,
};
use crate::infrastructure::daemon::rpc::DaemonRpcClient;
use crate::infrastructure::voicevox::{
    format_speakers_output, scan_available_models, AvailableModel, Speaker,
};
use crate::interface::cli::synthesis::flow::connect_daemon_rpc_auto_start;
use crate::interface::{AppOutput, StdAppOutput};

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
    match connect_daemon_rpc_auto_start(socket_path).await {
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
    if let Ok(dict_path) = crate::infrastructure::paths::find_openjtalk_dict() {
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

    if let Ok(onnx_path) = crate::infrastructure::paths::find_onnxruntime() {
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
    if let Ok(mut client) = DaemonRpcClient::new_at(socket_path).await {
        let (speakers, style_to_model) = client.list_speakers_with_models().await?;
        output.info(&format_speakers_output(
            "All available speakers and styles from daemon:",
            &speakers,
            Some(&style_to_model),
        ));
        return Ok(());
    }

    match connect_daemon_rpc_auto_start(socket_path).await {
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
