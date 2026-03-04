use anyhow::{Context, Result, anyhow};
use rodio::Player;
use std::{env, path::Path, sync::Arc};
use tokio::sync::oneshot;

use crate::interface::audio::{
    create_temp_wav_file, play_audio_from_memory, preferred_audio_players,
};

pub enum PlaybackOutcome {
    Completed,
    Cancelled(String),
}

pub struct PlaybackRequest<'a> {
    pub wav_data: &'a [u8],
    pub output_file: Option<&'a Path>,
    pub play: bool,
    pub cancel_rx: Option<oneshot::Receiver<String>>,
}

#[allow(clippy::future_not_send)]
pub async fn emit_and_play(request: PlaybackRequest<'_>) -> Result<PlaybackOutcome> {
    if let Some(output_file) = request.output_file {
        tokio::fs::write(output_file, request.wav_data).await?;
    }

    if !request.play {
        return Ok(PlaybackOutcome::Completed);
    }

    if let Some(mut cancel_rx) = request.cancel_rx {
        if env::var(crate::config::ENV_VOICEVOX_LOW_LATENCY).is_ok() {
            play_low_latency_with_cancel(request.wav_data.to_vec(), &mut cancel_rx).await
        } else {
            play_system_player_with_cancel(request.wav_data, &mut cancel_rx).await
        }
    } else {
        play_audio_from_memory(request.wav_data).context("Failed to play audio")?;
        Ok(PlaybackOutcome::Completed)
    }
}

#[allow(clippy::future_not_send)]
async fn play_low_latency_with_cancel(
    wav_data: Vec<u8>,
    cancel_rx: &mut oneshot::Receiver<String>,
) -> Result<PlaybackOutcome> {
    let stream = rodio::DeviceSinkBuilder::open_default_sink()
        .context("Failed to create audio output stream")?;
    let sink = Arc::new(Player::connect_new(stream.mixer()));
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
        result = cancel_rx => {
            match result {
                Ok(reason) => {
                    sink.stop();
                    let _ = playback_task.await;
                    Ok(PlaybackOutcome::Cancelled(reason))
                }
                Err(_) => {
                    playback_task.await.context("Audio playback task failed")??;
                    Ok(PlaybackOutcome::Completed)
                }
            }
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

    for command in preferred_audio_players() {
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
        result = cancel_rx => {
            match result {
                Ok(reason) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    Ok(Some(PlaybackOutcome::Cancelled(reason)))
                }
                Err(_) => {
                    let status = child.wait().await
                        .with_context(|| format!("Failed to wait for {command}"))?;
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
            }
        }
    }
}
