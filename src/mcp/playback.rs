use anyhow::{anyhow, Context, Result};
use rodio::Sink;
use std::{env, path::Path, sync::Arc};
use tokio::sync::oneshot;

use crate::client::audio::{create_temp_wav_file, play_audio_from_memory};

pub(crate) enum PlaybackOutcome {
    Completed,
    Cancelled(String),
}

pub(crate) fn append_wav_segments_to_sink(sink: &Sink, wav_segments: &[Vec<u8>]) -> Result<()> {
    sink.play();
    for (i, wav_data) in wav_segments.iter().enumerate() {
        let cursor = std::io::Cursor::new(wav_data.clone());
        let source = rodio::Decoder::new(cursor)
            .with_context(|| format!("Failed to decode audio for segment {i}"))?;
        sink.append(source);
    }
    Ok(())
}

#[allow(clippy::future_not_send)]
pub(crate) async fn wait_for_sink_with_cancellation(
    sink: Arc<Sink>,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<PlaybackOutcome> {
    let playback_task = tokio::task::spawn_blocking({
        let sink_for_task = Arc::clone(&sink);
        move || -> Result<()> {
            sink_for_task.sleep_until_end();
            Ok(())
        }
    });
    tokio::pin!(playback_task);

    if let Some(mut cancel_rx) = cancel_rx {
        tokio::select! {
            res = &mut playback_task => {
                res.context("Audio playback task failed")??;
                Ok(PlaybackOutcome::Completed)
            }
            reason = &mut cancel_rx => {
                let reason = reason.unwrap_or_default();
                sink.stop();
                let _ = playback_task.await;
                Ok(PlaybackOutcome::Cancelled(reason))
            }
        }
    } else {
        playback_task
            .await
            .context("Audio playback task failed")??;
        Ok(PlaybackOutcome::Completed)
    }
}

#[allow(clippy::future_not_send)]
pub(crate) async fn play_daemon_audio_with_cancellation(
    wav_data: Vec<u8>,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<PlaybackOutcome> {
    if let Some(mut cancel_rx) = cancel_rx {
        if env::var("VOICEVOX_LOW_LATENCY").is_ok() {
            play_low_latency_with_cancel(wav_data, &mut cancel_rx).await
        } else {
            play_system_player_with_cancel(&wav_data, &mut cancel_rx).await
        }
    } else {
        play_audio_from_memory(&wav_data).context("Failed to play audio")?;
        Ok(PlaybackOutcome::Completed)
    }
}

#[allow(clippy::future_not_send)]
async fn play_low_latency_with_cancel(
    wav_data: Vec<u8>,
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<PlaybackOutcome> {
    let stream = rodio::OutputStreamBuilder::open_default_stream()
        .context("Failed to create audio output stream")?;
    let sink = Arc::new(Sink::connect_new(stream.mixer()));
    let _stream_guard = stream;

    let cursor = std::io::Cursor::new(wav_data);
    let source = rodio::Decoder::new(cursor).context("Failed to decode audio")?;
    sink.append(source);
    sink.play();

    let playback_task = tokio::task::spawn_blocking({
        let sink_for_task = Arc::clone(&sink);
        move || -> Result<()> {
            sink_for_task.sleep_until_end();
            Ok(())
        }
    });
    tokio::pin!(playback_task);

    tokio::select! {
        res = &mut playback_task => {
            res.context("Audio playback task failed")??;
            Ok(PlaybackOutcome::Completed)
        }
        reason = cancel_rx => {
            let reason = reason.unwrap_or_default();
            sink.stop();
            let _ = playback_task.await;
            Ok(PlaybackOutcome::Cancelled(reason))
        }
    }
}

async fn play_system_player_with_cancel(
    wav_data: &[u8],
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<PlaybackOutcome> {
    let temp_file = create_temp_wav_file(wav_data)?;
    let temp_path = temp_file.path().to_owned();

    let mut last_error = None;

    for command in ["afplay", "play"] {
        match run_player_with_cancel(command, &temp_path, cancel_rx).await {
            Ok(Some(outcome)) => return Ok(outcome),
            Ok(None) => {}
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error
        .unwrap_or_else(|| anyhow!("No audio player found. Install sox or use -o to save file")))
}

async fn run_player_with_cancel(
    command: &str,
    temp_path: &Path,
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<Option<PlaybackOutcome>> {
    let mut child = match tokio::process::Command::new(command).arg(temp_path).spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("Failed to spawn {command}")),
    };

    tokio::select! {
        status = child.wait() => {
            let status = status.with_context(|| format!("Failed to wait for {command}"))?;
            if status.success() {
                Ok(Some(PlaybackOutcome::Completed))
            } else {
                Err(anyhow!(
                    "{command} exited with status {}",
                    status
                        .code()
                        .map_or_else(|| "terminated by signal".to_string(), |code| code.to_string())
                ))
            }
        }
        reason = cancel_rx => {
            let reason = reason.unwrap_or_default();
            let _ = child.kill().await;
            let _ = child.wait().await;
            Ok(Some(PlaybackOutcome::Cancelled(reason)))
        }
    }
}
