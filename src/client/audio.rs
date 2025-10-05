use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::process::Command;
use tempfile::{Builder, NamedTempFile};

pub fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    match play_audio_via_rodio(wav_data) {
        Ok(()) => Ok(()),
        Err(err) => match play_audio_via_system(wav_data) {
            Ok(()) => Ok(()),
            Err(system_err) => {
                Err(err.context(format!("System audio fallback failed: {system_err}")))
            }
        },
    }
}

fn play_audio_via_rodio(wav_data: &[u8]) -> Result<()> {
    use rodio::{Decoder, Sink};
    use std::io::Cursor;

    match rodio::OutputStreamBuilder::open_default_stream() {
        Ok(stream) => {
            let wav_data_owned = wav_data.to_vec();
            let cursor = Cursor::new(wav_data_owned);

            match Decoder::new(cursor) {
                Ok(source) => {
                    let sink = Sink::connect_new(stream.mixer());
                    sink.append(source);
                    sink.play();
                    sink.sleep_until_end();

                    // Explicitly drop to minimize debug output
                    drop(sink);
                    std::mem::drop(stream);
                    Ok(())
                }
                Err(_) => {
                    std::mem::drop(stream);
                    play_audio_via_system(wav_data)
                }
            }
        }
        Err(_) => play_audio_via_system(wav_data),
    }
}

fn play_audio_via_system(wav_data: &[u8]) -> Result<()> {
    let temp_file = create_temp_wav_file(wav_data)?;
    let temp_path = temp_file.path();

    if let Ok(output) = Command::new("afplay").arg(temp_path).output() {
        if output.status.success() {
            return Ok(());
        }
    }

    if let Ok(output) = Command::new("play").arg(temp_path).output() {
        if output.status.success() {
            return Ok(());
        }
    }

    Err(anyhow!(
        "No audio player found. Install sox or use -o to save file"
    ))
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
