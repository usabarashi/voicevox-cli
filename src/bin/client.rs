use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use voicevox_cli::client::{get_input_text, list_speakers_daemon, DaemonClient};
use voicevox_cli::core::{CoreSynthesis, VoicevoxCore};
use voicevox_cli::ipc::OwnedSynthesizeOptions;
use voicevox_cli::paths::get_socket_path;
use voicevox_cli::voice::{resolve_voice_dynamic, scan_available_models};

fn resolve_voice_from_args(matches: &clap::ArgMatches) -> Result<(u32, String)> {
    matches
        .get_one::<u32>("speaker-id")
        .map(|&id| (id, format!("Style ID {id}")))
        .or_else(|| {
            matches
                .get_one::<u32>("model")
                .map(|&id| (id, format!("Model {id} (Default Style)")))
        })
        .map(Ok)
        .or_else(|| {
            matches
                .get_one::<String>("voice")
                .map(|voice_name| resolve_voice_dynamic(voice_name))
        })
        .unwrap_or_else(|| Ok((3, "Default (Zundamon Normal)".to_string())))
}

async fn try_daemon_with_retry(
    text: &str,
    style_id: u32,
    options: OwnedSynthesizeOptions,
    output_file: Option<&String>,
    quiet: bool,
    _socket_path: &Path,
) -> Result<()> {
    if voicevox_cli::paths::find_models_dir().is_err() {
        return Err(voicevox_cli::daemon::DaemonError::NoModelsAvailable.into());
    }

    match DaemonClient::new_with_auto_start().await {
        Ok(mut client) => match client.synthesize(text, style_id, options).await {
            Ok(wav_data) => {
                if let Some(output_file) = output_file {
                    std::fs::write(output_file, &wav_data)?;
                }

                if !quiet && output_file.is_none() {
                    if let Err(e) = voicevox_cli::client::audio::play_audio_from_memory(&wav_data) {
                        eprintln!("Error: Audio playback failed: {e}");
                        return Err(e);
                    }
                }

                Ok(())
            }
            Err(e) => Err(e),
        },
        Err(e) => {
            if !quiet {
                match e.downcast_ref::<voicevox_cli::daemon::DaemonError>() {
                    Some(voicevox_cli::daemon::DaemonError::NoModelsAvailable) => {}
                    _ => {
                        eprintln!("Failed to connect to daemon: {}", e);
                    }
                }
            }
            Err(e)
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let app = Command::new("voicevox-say")
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
        );

    let matches = app.get_matches();

    if let Some(voice_name) = matches.get_one::<String>("voice") {
        if voice_name == "?" {
            resolve_voice_dynamic("?")?;
        }
    }

    if matches.get_flag("list-models") {
        let models = scan_available_models().unwrap_or_else(|e| {
            eprintln!("Error scanning models: {e}");
            std::process::exit(1);
        });

        if models.is_empty() {
            return Err(voicevox_cli::daemon::DaemonError::NoModelsAvailable.into());
        }

        println!("Available voice models:");
        for model in &models {
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

        return Ok(());
    }

    if matches.get_flag("status") {
        use voicevox_cli::paths::check_all_components;

        let components = check_all_components();
        let errors: Vec<_> = components
            .iter()
            .filter_map(|c| c.status.as_ref().err())
            .collect();
        println!("VOICEVOX CLI Installation Status");
        println!("=====================================");
        println!("Application: v{}", env!("CARGO_PKG_VERSION"));

        for component in &components {
            match &component.status {
                Ok(msg) => println!("{}: [OK] {}", component.name, msg),
                Err(e) => println!("{}: [ERROR] {}", component.name, e),
            }
            for detail in &component.details {
                println!("{}", detail);
            }
        }

        if !errors.is_empty() {
            println!("\n[!] Some components are missing. Run 'voicevox-setup' to install them.");
            return Err(anyhow!("Installation incomplete"));
        } else {
            println!("\n[✓] All components are properly installed.");
        }

        return Ok(());
    }

    if matches.get_flag("list-speakers") {
        let socket_path = matches
            .get_one::<String>("socket-path")
            .map(PathBuf::from)
            .unwrap_or_else(get_socket_path);

        if list_speakers_daemon(&socket_path).await.is_ok() {
            return Ok(());
        }

        let core = VoicevoxCore::new()?;

        let models = scan_available_models()?;
        if models.is_empty() {
            return Err(voicevox_cli::daemon::DaemonError::NoModelsAvailable.into());
        }

        for model in &models {
            if let Err(e) = core.load_specific_model(&model.model_id.to_string()) {
                println!("Warning: Failed to load model {}: {e}", model.model_id);
            }
        }

        println!("All available speakers and styles from loaded models:");
        let speakers = core.get_speakers()?;

        println!("Building style-to-model mapping...");
        let style_to_model: HashMap<u32, u32> = speakers
            .iter()
            .flat_map(|s| s.styles.iter().map(|style| (style.id, style.id)))
            .collect();

        for speaker in &speakers {
            println!("  {}", speaker.name);
            for style in &speaker.styles {
                let model_id = style_to_model.get(&style.id).copied().unwrap_or(style.id);
                println!(
                    "    {} (Model: {model_id}, Style ID: {})",
                    style.name, style.id
                );
                if let Some(style_type) = &style.style_type {
                    println!("        Type: {style_type}");
                }
            }
            println!();
        }
        return Ok(());
    }

    let text = get_input_text(&matches)?;
    if text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }

    let (style_id, _voice_description) = resolve_voice_from_args(&matches)?;

    let rate = *matches.get_one::<f32>("rate").unwrap_or(&1.0);
    let quiet = matches.get_flag("quiet");
    let output_file = matches.get_one::<String>("output-file");

    if !(0.5..=2.0).contains(&rate) {
        return Err(anyhow!("Rate must be between 0.5 and 2.0, got: {rate}"));
    }

    let options = OwnedSynthesizeOptions {
        rate,
        ..Default::default()
    };

    let socket_path = matches
        .get_one::<String>("socket-path")
        .map(PathBuf::from)
        .unwrap_or_else(get_socket_path);

    try_daemon_with_retry(&text, style_id, options, output_file, quiet, &socket_path).await
}
