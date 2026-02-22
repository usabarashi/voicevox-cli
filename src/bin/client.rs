use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::path::{Path, PathBuf};

use voicevox_cli::client::{
    emit_synthesized_audio, ensure_models_available, get_input_text, list_speakers_daemon,
    DaemonClient,
};
use voicevox_cli::ipc::{
    is_valid_synthesis_rate, OwnedSynthesizeOptions, DEFAULT_SYNTHESIS_RATE, MAX_SYNTHESIS_RATE,
    MIN_SYNTHESIS_RATE,
};
use voicevox_cli::paths::{find_openjtalk_dict, get_socket_path};
use voicevox_cli::voice::{print_voice_help, resolve_voice_dynamic, scan_available_models};

fn build_cli() -> Command {
    Command::new("voicevox-say")
        .version(env!("CARGO_PKG_VERSION"))
        .about("VOICEVOX Say - Convert text to audible speech using VOICEVOX")
        .arg(
            Arg::new("text")
                .help("Specify the text to speak on the command line")
                .index(1)
                .required(false),
        )
        .arg(
            Arg::new("voice")
                .help("Specify the voice to be used. Use '?' to list all available voices")
                .long("voice")
                .short('v')
                .value_name("VOICE"),
        )
        .arg(
            Arg::new("rate")
                .help("Speech rate multiplier (0.5-2.0, default: 1.0)")
                .long("rate")
                .short('r')
                .value_name("RATE")
                .value_parser(clap::value_parser!(f32))
                .default_value("1.0"),
        )
        .arg(
            Arg::new("output-file")
                .help("Specify the path for an audio file to be written")
                .long("output-file")
                .short('o')
                .value_name("FILE"),
        )
        .arg(
            Arg::new("input-file")
                .help("Specify a file to be spoken. Use '-' for stdin")
                .long("input-file")
                .short('f')
                .value_name("FILE"),
        )
        .arg(
            Arg::new("quiet")
                .help("Don't play audio, only save to file")
                .long("quiet")
                .short('q')
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("list-speakers")
                .help("List all available speakers and styles")
                .long("list-speakers")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("speaker-id")
                .help("Directly specify speaker style ID (advanced users)")
                .long("speaker-id")
                .value_name("ID")
                .value_parser(clap::value_parser!(u32))
                .conflicts_with_all(["voice", "model"]),
        )
        .arg(
            Arg::new("model")
                .help("Specify voice model by file number (e.g., --model 3 for 3.vvm)")
                .long("model")
                .short('m')
                .value_name("MODEL_ID")
                .value_parser(clap::value_parser!(u32))
                .conflicts_with_all(["voice", "speaker-id"]),
        )
        .arg(
            Arg::new("list-models")
                .help("List all available voice models and exit")
                .long("list-models")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("status")
                .help("Show installation status of voice models and dictionary")
                .long("status")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("socket-path")
                .help("Specify custom Unix socket path")
                .long("socket-path")
                .short('S')
                .value_name("PATH"),
        )
}

const NO_MODELS_MESSAGE: &str =
    "No voice models found. Please run 'voicevox-setup' to download required resources.";

fn socket_path_from_matches(matches: &clap::ArgMatches) -> PathBuf {
    matches
        .get_one::<String>("socket-path")
        .map_or_else(get_socket_path, PathBuf::from)
}

fn handle_voice_help_request(matches: &clap::ArgMatches) -> bool {
    if matches
        .get_one::<String>("voice")
        .is_some_and(|voice_name| voice_name == "?")
    {
        print_voice_help();
        return true;
    }
    false
}

fn print_no_models_message() {
    println!("{NO_MODELS_MESSAGE}");
}

fn handle_missing_models_error(error: anyhow::Error) -> Result<bool> {
    if voicevox_cli::paths::find_models_dir().is_err() {
        print_no_models_message();
        return Ok(true);
    }

    Err(error)
}

const DEFAULT_STYLE_ID: u32 = 3;

const fn default_voice_selection() -> u32 {
    DEFAULT_STYLE_ID
}

fn print_list_models_output(models: &[voicevox_cli::voice::AvailableModel]) {
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
    }

    println!("\nTips:");
    println!("  - Use --model N to load model N.vvm");
    println!("  - Use --speaker-id for direct style ID specification");
    println!("  - Use --list-speakers for detailed speaker information");
}

async fn handle_list_models_command(matches: &clap::ArgMatches) -> Result<bool> {
    let socket_path = socket_path_from_matches(matches);

    match DaemonClient::new_with_auto_start_at(&socket_path).await {
        Ok(mut client) => {
            let models = client.list_models().await?;
            print_list_models_output(&models);
        }
        Err(error) => {
            return handle_missing_models_error(error);
        }
    }
    Ok(true)
}

fn print_status_models(current_models: &[voicevox_cli::voice::AvailableModel]) {
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

fn handle_status_command() -> bool {
    println!("VOICEVOX CLI Installation Status");
    println!("=====================================");

    println!("Application: v{}", env!("CARGO_PKG_VERSION"));

    if let Ok(onnx_path) = voicevox_cli::paths::find_onnxruntime() {
        println!("ONNX Runtime: {}", onnx_path.display());
    } else {
        print_missing_status_item("ONNX Runtime");
    }

    match scan_available_models() {
        Ok(current_models) => {
            print_status_models(&current_models);
            print_status_dictionary();
        }
        Err(e) => {
            print_missing_status_item("Voice Models");
            eprintln!("Error scanning models: {e}");
        }
    }

    true
}

fn print_speakers(speakers: &[voicevox_cli::voice::Speaker]) {
    println!("All available speakers and styles:");
    for speaker in speakers {
        println!("  {}", speaker.name);
        for style in &speaker.styles {
            println!("    {} (Style ID: {})", style.name, style.id);
            if let Some(style_type) = &style.style_type {
                println!("        Type: {style_type}");
            }
        }
        println!();
    }
}

async fn handle_list_speakers_command(matches: &clap::ArgMatches) -> Result<bool> {
    let socket_path = socket_path_from_matches(matches);

    if list_speakers_daemon(&socket_path).await.is_ok() {
        return Ok(true);
    }

    match DaemonClient::new_with_auto_start_at(&socket_path).await {
        Ok(mut client) => {
            let speakers = client.list_speakers().await?;
            print_speakers(&speakers);
        }
        Err(error) => {
            return handle_missing_models_error(error);
        }
    }

    Ok(true)
}

enum MetaCommand {
    ListModels,
    Status,
    ListSpeakers,
}

fn selected_meta_command(matches: &clap::ArgMatches) -> Option<MetaCommand> {
    if matches.get_flag("list-models") {
        Some(MetaCommand::ListModels)
    } else if matches.get_flag("status") {
        Some(MetaCommand::Status)
    } else if matches.get_flag("list-speakers") {
        Some(MetaCommand::ListSpeakers)
    } else {
        None
    }
}

async fn maybe_handle_meta_commands(matches: &clap::ArgMatches) -> Result<bool> {
    match selected_meta_command(matches) {
        Some(MetaCommand::ListModels) => handle_list_models_command(matches).await,
        Some(MetaCommand::Status) => Ok(handle_status_command()),
        Some(MetaCommand::ListSpeakers) => handle_list_speakers_command(matches).await,
        None => Ok(false),
    }
}

async fn run_synthesis_command(matches: &clap::ArgMatches) -> Result<()> {
    let text = get_input_text(matches)?;
    if text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }

    let style_id = resolve_voice_from_args(matches)?;
    let rate = matches
        .get_one::<f32>("rate")
        .copied()
        .unwrap_or(DEFAULT_SYNTHESIS_RATE);
    let quiet = matches.get_flag("quiet");
    let output_file = matches.get_one::<String>("output-file").map(Path::new);
    if !is_valid_synthesis_rate(rate) {
        return Err(anyhow!(
            "Rate must be between {MIN_SYNTHESIS_RATE:.1} and {MAX_SYNTHESIS_RATE:.1}, got: {rate}"
        ));
    }

    let socket_path = socket_path_from_matches(matches);
    let options = OwnedSynthesizeOptions { rate };

    try_daemon_with_retry(&text, style_id, options, output_file, quiet, &socket_path).await
}

fn resolve_voice_from_args(matches: &clap::ArgMatches) -> Result<u32> {
    if let Some(&id) = matches.get_one::<u32>("speaker-id") {
        return Ok(id);
    }

    if let Some(&id) = matches.get_one::<u32>("model") {
        return Ok(id);
    }

    if let Some(voice_name) = matches.get_one::<String>("voice") {
        return resolve_voice_dynamic(voice_name).map(|(style_id, _description)| style_id);
    }

    Ok(default_voice_selection())
}

async fn try_daemon_with_retry(
    text: &str,
    style_id: u32,
    options: OwnedSynthesizeOptions,
    output_file: Option<&Path>,
    quiet: bool,
    socket_path: &Path,
) -> Result<()> {
    if voicevox_cli::paths::find_models_dir().is_err() {
        if !quiet {
            println!("Voice models not found. Setting up VOICEVOX...");
        }
        ensure_models_available().await?;
    }

    match DaemonClient::new_with_auto_start_at(socket_path).await {
        Ok(mut client) => {
            let wav_data = client.synthesize(text, style_id, options).await?;

            emit_synthesized_audio(&wav_data, output_file, quiet)?;
            Ok(())
        }
        Err(e) => {
            if !quiet {
                eprintln!("Failed to connect to daemon: {e}");
            }
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = build_cli().get_matches();
    if handle_voice_help_request(&matches) {
        return Ok(());
    }
    if maybe_handle_meta_commands(&matches).await? {
        return Ok(());
    }
    run_synthesis_command(&matches).await
}
