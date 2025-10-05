use anyhow::{anyhow, Result};
use clap::{Arg, Command};
use std::path::{Path, PathBuf};

use voicevox_cli::client::{
    ensure_models_available, get_input_text, list_speakers_daemon, play_audio_from_memory,
    DaemonClient,
};
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
        if !quiet {
            println!("Voice models not found. Setting up VOICEVOX...");
        }
        ensure_models_available().await?;
    }

    match DaemonClient::new_with_auto_start().await {
        Ok(mut client) => {
            let wav_data = client.synthesize(text, style_id, options).await?;

            if let Some(output_file) = output_file {
                std::fs::write(output_file, &wav_data)?;
            }

            if !quiet && output_file.is_none() {
                play_audio_from_memory(wav_data.clone())?;
            }

            Ok(())
        }
        Err(e) => {
            if !quiet {
                eprintln!("Failed to connect to daemon: {}", e);
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
                .help("Write synthesized audio to the specified WAV file")
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
                .help("Suppress playback (use with --output-file to export silently)")
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
        match DaemonClient::new_with_auto_start().await {
            Ok(mut client) => {
                let models = client.list_models().await?;
                if models.is_empty() {
                    println!("No voice models found. Please run 'voicevox-setup' to download required resources.");
                } else {
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
                }
            }
            Err(_) => {
                println!("No voice models found. Please run 'voicevox-setup' to download required resources.");
            }
        }
        return Ok(());
    }

    if matches.get_flag("status") {
        println!("VOICEVOX CLI Installation Status");
        println!("=====================================");

        println!("Application: v{}", env!("CARGO_PKG_VERSION"));

        match voicevox_cli::paths::find_onnxruntime() {
            Ok(onnx_path) => {
                println!("ONNX Runtime: {}", onnx_path.display());
            }
            Err(_) => {
                println!("ONNX Runtime: Not found");
                println!("  Install with: voicevox-setup");
            }
        }

        match scan_available_models() {
            Ok(current_models) => {
                if current_models.is_empty() {
                    println!("Voice Models: Not found");
                    println!("  Install with: voicevox-setup");
                } else {
                    println!("Voice Models: {} files installed", current_models.len());
                    for model in &current_models {
                        let model_info = match std::fs::metadata(&model.file_path) {
                            Ok(metadata) => {
                                let size_kb = metadata.len() / 1024;
                                let filename = model
                                    .file_path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy();
                                format!("  Model {}: {filename} ({size_kb} KB)", model.model_id)
                            }
                            Err(_) => {
                                format!(
                                    "  Model {} ({})",
                                    model.model_id,
                                    model.file_path.display()
                                )
                            }
                        };
                        println!("{model_info}");
                    }
                }

                use voicevox_cli::paths::find_openjtalk_dict;
                match find_openjtalk_dict() {
                    Ok(dict_path) => {
                        println!("Dictionary: {}", dict_path.display());
                    }
                    Err(_) => {
                        println!("Dictionary: Not found");
                        println!("  Install with: voicevox-setup");
                    }
                }
            }
            Err(e) => {
                println!("Voice Models: Not found");
                println!("  Install with: voicevox-setup");
                eprintln!("Error scanning models: {e}");
            }
        }
        return Ok(());
    }

    if matches.get_flag("list-speakers") {
        let socket_path = matches
            .get_one::<String>("socket-path")
            .map(PathBuf::from)
            .unwrap_or_else(get_socket_path);

        // Try to connect to daemon first
        if list_speakers_daemon(&socket_path).await.is_ok() {
            return Ok(());
        }

        // If daemon connection fails, try to start daemon automatically
        match DaemonClient::new_with_auto_start().await {
            Ok(mut client) => {
                let speakers = client.list_speakers().await?;
                println!("All available speakers and styles:");
                for speaker in &speakers {
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
            Err(_) => {
                println!("No voice models found. Please run 'voicevox-setup' to download required resources.");
            }
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

    let options = OwnedSynthesizeOptions { rate };

    let socket_path = matches
        .get_one::<String>("socket-path")
        .map(PathBuf::from)
        .unwrap_or_else(get_socket_path);

    try_daemon_with_retry(
        &text,
        style_id,
        options.clone(),
        output_file,
        quiet,
        &socket_path,
    )
    .await
}
