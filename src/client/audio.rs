use anyhow::{anyhow, Result};
use rodio::{Decoder, OutputStream, Sink};
use std::fs;
use std::io::Cursor;

// Direct audio playback from memory (like macOS say command)
pub fn play_audio_from_memory(wav_data: &[u8]) -> Result<()> {
    // Try rodio first for cross-platform compatibility
    if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
        let sink = Sink::try_new(&stream_handle)?;
        // Create owned data for Decoder to avoid lifetime issues
        let wav_data_owned = wav_data.to_vec();
        let cursor = Cursor::new(wav_data_owned);

        match Decoder::new(cursor) {
            Ok(source) => {
                sink.append(source);
                sink.sleep_until_end();
                return Ok(());
            }
            Err(_) => {
                // Rodio failed, fall back to system command
            }
        }
    }

    // Fallback to system audio players (like original say command behavior)
    play_audio_via_system(wav_data)
}

// System audio playback fallback
fn play_audio_via_system(wav_data: &[u8]) -> Result<()> {
    let temp_file = "/tmp/voicevox_say_temp.wav";
    fs::write(temp_file, wav_data)?;

    // Auto-cleanup guard
    struct TempFileCleanup<'a>(&'a str);
    impl Drop for TempFileCleanup<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }
    let _cleanup = TempFileCleanup(temp_file);

    // macOS standard afplay for playback (silent like say command)
    if std::process::Command::new("afplay")
        .arg(temp_file)
        .output()
        .is_ok()
    {
        return Ok(());
    }

    // sox fallback
    if std::process::Command::new("play")
        .arg(temp_file)
        .output()
        .is_ok()
    {
        return Ok(());
    }

    Err(anyhow!(
        "No audio player found. Install sox or use -o to save file"
    ))
}
