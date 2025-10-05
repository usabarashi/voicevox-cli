use anyhow::{Context, Result};

/// Play the provided WAV data entirely in memory via rodio.
pub fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    use rodio::{Decoder, Sink};
    use std::io::Cursor;

    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;
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
