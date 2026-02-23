use anyhow::{anyhow, Result};
use clap::{ArgGroup, Parser};
use std::path::{Path, PathBuf};

use voicevox_cli::client::{
    emit_synthesized_audio, ensure_models_available, get_input_text_from_sources,
    list_speakers_daemon, DaemonClient,
};
use voicevox_cli::ipc::{
    is_valid_synthesis_rate, OwnedSynthesizeOptions, DEFAULT_SYNTHESIS_RATE, MAX_SYNTHESIS_RATE,
    MIN_SYNTHESIS_RATE,
};
use voicevox_cli::paths::{find_openjtalk_dict, get_socket_path};
use voicevox_cli::voice::{print_voice_help, resolve_voice_dynamic, scan_available_models};

// Clap option flags are intentionally represented as booleans.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Parser)]
#[command(
    name = "voicevox-say",
    version,
    about = "VOICEVOX Say - Convert text to audible speech using VOICEVOX",
    group(
        ArgGroup::new("meta_command")
            .args(["list_speakers", "list_models", "status"])
            .multiple(false)
    )
)]
struct CliArgs {
    #[arg(help = "Specify the text to speak on the command line", index = 1)]
    text: Option<String>,

    #[arg(
        long,
        short = 'v',
        value_name = "VOICE",
        help = "Specify the voice to be used. Use '?' to list all available voices",
        conflicts_with_all = ["speaker_id", "model"]
    )]
    voice: Option<String>,

    #[arg(
        long,
        short = 'r',
        value_name = "RATE",
        default_value_t = DEFAULT_SYNTHESIS_RATE,
        help = "Speech rate multiplier (0.5-2.0, default: 1.0)"
    )]
    rate: f32,

    #[arg(long = "output-file", short = 'o', value_name = "FILE")]
    output_file: Option<PathBuf>,

    #[arg(long = "input-file", short = 'f', value_name = "FILE")]
    input_file: Option<String>,

    #[arg(long, short = 'q', help = "Don't play audio, only save to file")]
    quiet: bool,

    #[arg(
        long = "list-speakers",
        help = "List all available speakers and styles"
    )]
    list_speakers: bool,

    #[arg(
        long = "speaker-id",
        value_name = "ID",
        help = "Directly specify speaker style ID (advanced users)",
        conflicts_with_all = ["voice", "model"]
    )]
    speaker_id: Option<u32>,

    #[arg(
        long,
        short = 'm',
        value_name = "MODEL_ID",
        help = "Specify voice model by file number (e.g., --model 3 for 3.vvm)",
        conflicts_with_all = ["voice", "speaker_id"]
    )]
    model: Option<u32>,

    #[arg(
        long = "list-models",
        help = "List all available voice models and exit"
    )]
    list_models: bool,

    #[arg(long, help = "Show installation status of voice models and dictionary")]
    status: bool,

    #[arg(long = "socket-path", short = 'S', value_name = "PATH")]
    socket_path: Option<PathBuf>,
}

impl CliArgs {
    fn socket_path(&self) -> PathBuf {
        self.socket_path.clone().unwrap_or_else(get_socket_path)
    }

    fn wants_voice_help(&self) -> bool {
        self.voice.as_deref() == Some("?")
    }

    fn selected_meta_command(&self) -> Option<MetaCommand> {
        if self.list_models {
            Some(MetaCommand::ListModels)
        } else if self.status {
            Some(MetaCommand::Status)
        } else if self.list_speakers {
            Some(MetaCommand::ListSpeakers)
        } else {
            None
        }
    }
}

const NO_MODELS_MESSAGE: &str =
    "No voice models found. Please run 'voicevox-setup' to download required resources.";

fn handle_voice_help_request(args: &CliArgs) -> bool {
    if args.wants_voice_help() {
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

async fn handle_list_models_command(args: &CliArgs) -> Result<bool> {
    let socket_path = args.socket_path();

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

async fn handle_list_speakers_command(args: &CliArgs) -> Result<bool> {
    let socket_path = args.socket_path();

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

enum VoiceSelection<'a> {
    SpeakerId(u32),
    ModelId(u32),
    VoiceName(&'a str),
    Default,
}

impl<'a> VoiceSelection<'a> {
    fn from_args(args: &'a CliArgs) -> Self {
        if let Some(id) = args.speaker_id {
            Self::SpeakerId(id)
        } else if let Some(id) = args.model {
            Self::ModelId(id)
        } else if let Some(voice_name) = args.voice.as_deref() {
            Self::VoiceName(voice_name)
        } else {
            Self::Default
        }
    }
}

async fn maybe_handle_meta_commands(args: &CliArgs) -> Result<bool> {
    match args.selected_meta_command() {
        Some(MetaCommand::ListModels) => handle_list_models_command(args).await,
        Some(MetaCommand::Status) => Ok(handle_status_command()),
        Some(MetaCommand::ListSpeakers) => handle_list_speakers_command(args).await,
        None => Ok(false),
    }
}

async fn run_synthesis_command(args: &CliArgs) -> Result<()> {
    let text = get_input_text_from_sources(args.text.as_deref(), args.input_file.as_deref())?;
    if text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }

    let style_id = resolve_voice_from_args(args)?;
    let rate = args.rate;
    let quiet = args.quiet;
    let output_file = args.output_file.as_deref();
    if !is_valid_synthesis_rate(rate) {
        return Err(anyhow!(
            "Rate must be between {MIN_SYNTHESIS_RATE:.1} and {MAX_SYNTHESIS_RATE:.1}, got: {rate}"
        ));
    }

    let socket_path = args.socket_path();
    let options = OwnedSynthesizeOptions { rate };

    try_daemon_with_retry(&text, style_id, options, output_file, quiet, &socket_path).await
}

fn resolve_voice_from_args(args: &CliArgs) -> Result<u32> {
    match VoiceSelection::from_args(args) {
        VoiceSelection::SpeakerId(id) | VoiceSelection::ModelId(id) => Ok(id),
        VoiceSelection::VoiceName(voice_name) => {
            resolve_voice_dynamic(voice_name).map(|(style_id, _description)| style_id)
        }
        VoiceSelection::Default => Ok(default_voice_selection()),
    }
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
    let args = CliArgs::parse();
    if handle_voice_help_request(&args) {
        return Ok(());
    }
    if maybe_handle_meta_commands(&args).await? {
        return Ok(());
    }
    run_synthesis_command(&args).await
}
