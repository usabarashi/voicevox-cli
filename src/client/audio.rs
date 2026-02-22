use anyhow::{anyhow, Context, Result};
use std::path::Path;
use std::process::Command;
use std::{env, io::Write};
use tempfile::{Builder, NamedTempFile};

/// Plays synthesized WAV audio from memory using rodio or a system player fallback.
///
/// # Errors
///
/// Returns an error if audio decoding/playback fails and no compatible system player
/// (such as `afplay` or `play`) succeeds.
pub fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    if env::var("VOICEVOX_LOW_LATENCY").is_ok() {
        play_audio_via_rodio(wav_data)
    } else {
        play_audio_via_system(wav_data)
    }
}

/// Writes WAV output and optionally plays it back.
///
/// # Errors
///
/// Returns an error if file writing fails or audio playback fails.
pub fn emit_synthesized_audio(
    wav_data: &[u8],
    output_file: Option<&Path>,
    quiet: bool,
) -> Result<()> {
    if let Some(output_file) = output_file {
        std::fs::write(output_file, wav_data)?;
    }

    if !quiet && output_file.is_none() {
        play_audio_from_memory(wav_data)
            .inspect_err(|e| eprintln!("Error: Audio playback failed: {e}"))?;
    }

    Ok(())
}

fn play_audio_via_rodio(wav_data: &[u8]) -> Result<()> {
    use rodio::{Decoder, Sink};
    use std::io::Cursor;

    rodio::OutputStreamBuilder::open_default_stream().map_or_else(
        |_| play_audio_via_system(wav_data),
        |stream| {
            let wav_data_owned = wav_data.to_vec();
            let cursor = Cursor::new(wav_data_owned);
            Decoder::new(cursor).map_or_else(
                |_| play_audio_via_system(wav_data),
                |source| {
                    let sink = Sink::connect_new(stream.mixer());
                    sink.append(source);
                    sink.play();
                    sink.sleep_until_end();
                    Ok(())
                },
            )
        },
    )
}

fn play_audio_via_system(wav_data: &[u8]) -> Result<()> {
    let temp_file = create_temp_wav_file(wav_data)?;
    let temp_path = temp_file.path();

    if ["afplay", "play"]
        .into_iter()
        .any(|cmd| try_system_player(cmd, temp_path))
    {
        return Ok(());
    }

    Err(anyhow!(
        "No audio player found. Install sox or use -o to save file"
    ))
}

fn try_system_player(command: &str, temp_path: &std::path::Path) -> bool {
    matches!(
        Command::new(command).arg(temp_path).output(),
        Ok(output) if output.status.success()
    )
}

pub(crate) fn create_temp_wav_file(wav_data: &[u8]) -> Result<NamedTempFile> {
    let mut temp = Builder::new()
        .prefix("voicevox_")
        .suffix(".wav")
        .tempfile()
        .context("Failed to create temporary audio file")?;

    temp.write_all(wav_data)
        .context("Failed to write temporary audio file")?;
    temp.flush()
        .context("Failed to flush temporary audio file")?;

    Ok(temp)
}
