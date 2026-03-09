use anyhow::{Context, Result, anyhow};
use std::process::Command;
use std::{env, io::Write};
use tempfile::{Builder, NamedTempFile};

pub(crate) fn preferred_audio_players() -> Vec<&'static str> {
    let mut players = Vec::new();
    for path in crate::config::SYSTEM_AUDIO_PLAYER_PATHS {
        if std::path::Path::new(path).is_file() {
            players.push(path);
        }
    }
    if crate::config::allow_unsafe_path_commands() {
        players.extend(crate::config::FALLBACK_AUDIO_PLAYERS);
    }
    players
}

/// Plays synthesized WAV audio from memory using rodio or a system player fallback.
///
/// # Errors
///
/// Returns an error if audio decoding/playback fails and no compatible system player
/// (such as `afplay` or `play`) succeeds.
pub fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    if env::var(crate::config::ENV_VOICEVOX_LOW_LATENCY).is_ok() {
        play_audio_via_rodio(wav_data)
    } else {
        play_audio_via_system(wav_data)
    }
}

fn play_audio_via_rodio(wav_data: &[u8]) -> Result<()> {
    use rodio::{Decoder, Player};
    use std::io::Cursor;

    let Ok(stream) = rodio::DeviceSinkBuilder::open_default_sink() else {
        return play_audio_via_system(wav_data);
    };

    let Ok(source) = Decoder::new(Cursor::new(wav_data.to_vec())) else {
        return play_audio_via_system(wav_data);
    };

    let sink = Player::connect_new(stream.mixer());
    sink.append(source);
    sink.play();
    sink.sleep_until_end();
    Ok(())
}

fn play_audio_via_system(wav_data: &[u8]) -> Result<()> {
    let temp_file = create_temp_wav_file(wav_data)?;
    let temp_path = temp_file.path();

    try_players(preferred_audio_players(), |command| {
        try_system_player(command, temp_path)
    })
}

fn try_players<I, F>(commands: I, mut try_command: F) -> Result<()>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
    F: FnMut(&str) -> Result<Option<()>>,
{
    let mut last_error = None;

    for command in commands {
        match try_command(command.as_ref()) {
            Ok(Some(())) => return Ok(()),
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error
        .unwrap_or_else(|| anyhow!("No audio player found. Install sox or use -o to save file")))
}

fn try_system_player(command: &str, temp_path: &std::path::Path) -> Result<Option<()>> {
    let output = match Command::new(command).arg(temp_path).output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("Failed to launch {command}")),
    };

    if output.status.success() {
        return Ok(Some(()));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let message = stderr.trim();
    if message.is_empty() {
        Err(anyhow!(
            "{command} exited with status {}",
            output.status.code().map_or_else(
                || "terminated by signal".to_string(),
                |code| code.to_string()
            )
        ))
    } else {
        Err(anyhow!("{command} failed: {message}"))
    }
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
