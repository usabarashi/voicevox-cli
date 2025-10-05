use anyhow::{anyhow, Context, Result};
use std::io::Write;
use std::process::Command;
use tempfile::{Builder, NamedTempFile};

pub fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    play_audio_via_rodio(wav_data).or_else(|rodio_err| {
        play_audio_via_system(wav_data)
            .map_err(|system_err| map_system_fallback_error(system_err, rodio_err))
    })
}

fn play_audio_via_rodio(wav_data: &[u8]) -> Result<()> {
    use rodio::{Decoder, Sink};
    use std::io::Cursor;

    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;
    // rodio::Sink::append requires `Source + Send + 'static`. `Cursor<&[u8]>` fails this bound and
    // triggers E0521 (borrowed data escapes). Cloning here gives us an owned buffer whose lifetime
    // cleanly spans the playback.
    let cursor = Cursor::new(wav_data.to_vec());
    let source = Decoder::new(cursor).context("Failed to decode audio")?;
    let sink = Sink::connect_new(stream.mixer());
    sink.append(source);
    sink.play();
    sink.sleep_until_end();

    drop(sink);
    std::mem::drop(stream);
    Ok(())
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

pub(crate) fn map_system_fallback_error(
    system_err: anyhow::Error,
    rodio_err: anyhow::Error,
) -> anyhow::Error {
    system_err.context(format!("Low-latency audio playback failed: {rodio_err}"))
}
