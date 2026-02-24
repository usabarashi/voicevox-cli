use anyhow::Result;
use std::path::Path;

use crate::client::{list_speakers_daemon, DaemonClient};
use crate::paths::find_openjtalk_dict;
use crate::voice::{format_speakers_output, scan_available_models, AvailableModel, Speaker};

const NO_MODELS_MESSAGE: &str =
    "No voice models found. Please run 'voicevox-setup' to download required resources.";

fn print_no_models_message() {
    println!("{NO_MODELS_MESSAGE}");
}

fn handle_missing_models_error(error: anyhow::Error) -> Result<()> {
    if crate::paths::find_models_dir().is_err() {
        print_no_models_message();
        return Ok(());
    }

    Err(error)
}

fn print_list_models_output(models: &[AvailableModel]) {
    if models.is_empty() {
        print_no_models_message();
        return;
    }

    println!("Available voice models:");
    for model in models {
        println!("  Model {} ({})", model.model_id, model.file_path.display());
        println!(
            "    Usage: --model {} or --speaker-id <STYLE_ID>",
            model.model_id
        );
        if let Some(default_style_id) = model
            .speakers
            .iter()
            .flat_map(|speaker| speaker.styles.iter())
            .map(|style| style.id)
            .min()
        {
            println!("    Default style ID (auto-selected by --model): {default_style_id}");
        }
    }

    println!("\nTips:");
    println!("  - Use --model N to load model N.vvm");
    println!("  - Use --speaker-id for direct style ID specification");
    println!("  - Use --list-speakers for detailed speaker information");
}

pub async fn run_list_models_command(socket_path: &Path) -> Result<()> {
    match DaemonClient::new_with_auto_start_at(socket_path).await {
        Ok(mut client) => {
            let models = client.list_models().await?;
            print_list_models_output(&models);
            Ok(())
        }
        Err(error) => handle_missing_models_error(error),
    }
}

fn print_status_models(current_models: &[AvailableModel]) {
    if current_models.is_empty() {
        print_missing_status_item("Voice Models");
        return;
    }

    println!("Voice Models: {} files installed", current_models.len());
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
        println!("{model_info}");
    }
}

fn print_missing_status_item(name: &str) {
    println!("{name}: Not found");
    println!("  Install with: voicevox-setup");
}

fn print_status_dictionary() {
    if let Ok(dict_path) = find_openjtalk_dict() {
        println!("Dictionary: {}", dict_path.display());
    } else {
        print_missing_status_item("Dictionary");
    }
}

pub fn run_status_command() {
    println!("VOICEVOX CLI Installation Status");
    println!("=====================================");
    println!("Application: v{}", env!("CARGO_PKG_VERSION"));

    if let Ok(onnx_path) = crate::paths::find_onnxruntime() {
        println!("ONNX Runtime: {}", onnx_path.display());
    } else {
        print_missing_status_item("ONNX Runtime");
    }

    match scan_available_models() {
        Ok(current_models) => {
            print_status_models(&current_models);
            print_status_dictionary();
        }
        Err(error) => {
            print_missing_status_item("Voice Models");
            eprintln!("Error scanning models: {error}");
        }
    }
}

fn print_speakers(speakers: &[Speaker]) {
    println!(
        "{}",
        format_speakers_output("All available speakers and styles:", speakers, None)
    );
}

pub async fn run_list_speakers_command(socket_path: &Path) -> Result<()> {
    if list_speakers_daemon(socket_path).await.is_ok() {
        return Ok(());
    }

    match DaemonClient::new_with_auto_start_at(socket_path).await {
        Ok(mut client) => {
            let speakers = client.list_speakers().await?;
            print_speakers(&speakers);
            Ok(())
        }
        Err(error) => handle_missing_models_error(error),
    }
}
