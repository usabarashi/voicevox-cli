use anyhow::Result;
use clap::{ArgGroup, Parser};
use std::path::PathBuf;
use std::process::ExitCode;

use voicevox_cli::infrastructure::paths::get_socket_path;
use voicevox_cli::infrastructure::voicevox::{print_voice_help, resolve_voice_dynamic};
use voicevox_cli::interface::cli::{
    daemon_rpc_exit_code, find_daemon_rpc_error, format_daemon_rpc_error_for_cli,
    get_input_text_from_sources,
};
use voicevox_cli::interface::cli::{
    run_list_models_command, run_list_speakers_command, run_say_synthesis, run_status_command,
    SaySynthesisRequest,
};
use voicevox_cli::interface::ipc::DEFAULT_SYNTHESIS_RATE;

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

fn handle_voice_help_request(args: &CliArgs) -> bool {
    if args.wants_voice_help() {
        print_voice_help();
        return true;
    }
    false
}

const DEFAULT_STYLE_ID: u32 = 3;

const fn default_voice_selection() -> u32 {
    DEFAULT_STYLE_ID
}

async fn handle_list_models_command(args: &CliArgs) -> Result<bool> {
    run_list_models_command(&args.socket_path()).await?;
    Ok(true)
}

fn handle_status_command() -> bool {
    run_status_command();
    true
}

async fn handle_list_speakers_command(args: &CliArgs) -> Result<bool> {
    run_list_speakers_command(&args.socket_path()).await?;
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
    let style_id = resolve_voice_from_args(args)?;
    run_say_synthesis(SaySynthesisRequest {
        text: &text,
        style_id,
        rate: args.rate,
        output_file: args.output_file.as_deref(),
        quiet: args.quiet,
        socket_path: args.socket_path(),
    })
    .await
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

async fn run_client_command(args: &CliArgs) -> Result<()> {
    if handle_voice_help_request(args) {
        return Ok(());
    }
    if maybe_handle_meta_commands(args).await? {
        return Ok(());
    }
    run_synthesis_command(args).await
}

fn should_print_error_in_main(args: &CliArgs, error: &anyhow::Error) -> bool {
    if find_daemon_rpc_error(error).is_none() {
        return true;
    }

    args.quiet || args.selected_meta_command().is_some()
}

fn print_cli_error(args: &CliArgs, error: &anyhow::Error) {
    if !should_print_error_in_main(args, error) {
        return;
    }

    if find_daemon_rpc_error(error).is_some() {
        eprintln!("{}", format_daemon_rpc_error_for_cli(error));
    } else {
        eprintln!("Error: {error}");
    }
}

fn exit_code_for_error(error: &anyhow::Error) -> ExitCode {
    ExitCode::from(daemon_rpc_exit_code(error).unwrap_or(1))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = CliArgs::parse();
    match run_client_command(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            print_cli_error(&args, &error);
            exit_code_for_error(&error)
        }
    }
}
