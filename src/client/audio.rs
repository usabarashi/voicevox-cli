use anyhow::{anyhow, Result};
use std::fs;

pub fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    if std::env::var("VOICEVOX_LOW_LATENCY").is_ok() {
        play_audio_via_rodio(wav_data)
    } else {
        play_audio_via_system(wav_data)
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
    let temp_file = "/tmp/voicevox_say_temp.wav";
    fs::write(temp_file, wav_data)?;

    struct TempFileCleanup<'a>(&'a str);
    impl Drop for TempFileCleanup<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }
    let _cleanup = TempFileCleanup(temp_file);

    if let Ok(output) = std::process::Command::new("afplay").arg(temp_file).output() {
        if output.status.success() {
            return Ok(());
        }
    }

    if let Ok(output) = std::process::Command::new("play").arg(temp_file).output() {
        if output.status.success() {
            return Ok(());
        }
    }

    Err(anyhow!(
        "No audio player found. Install sox or use -o to save file"
    ))
}
