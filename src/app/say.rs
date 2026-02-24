use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

use crate::client::{emit_synthesized_audio, ensure_models_available, DaemonClient};
use crate::ipc::{
    is_valid_synthesis_rate, OwnedSynthesizeOptions, MAX_SYNTHESIS_RATE, MIN_SYNTHESIS_RATE,
};

pub struct SaySynthesisRequest<'a> {
    pub text: &'a str,
    pub style_id: u32,
    pub rate: f32,
    pub output_file: Option<&'a Path>,
    pub quiet: bool,
    pub socket_path: PathBuf,
}

/// Runs the main CLI synthesis use case against the daemon, including setup-on-demand.
///
/// # Errors
///
/// Returns an error if validation fails, setup fails, daemon connection fails, or playback/write fails.
pub async fn run_say_synthesis(request: SaySynthesisRequest<'_>) -> Result<()> {
    if request.text.trim().is_empty() {
        return Err(anyhow!(
            "No text provided. Use command line argument, -f file, or pipe text to stdin."
        ));
    }

    if !is_valid_synthesis_rate(request.rate) {
        return Err(anyhow!(
            "Rate must be between {MIN_SYNTHESIS_RATE:.1} and {MAX_SYNTHESIS_RATE:.1}, got: {}",
            request.rate
        ));
    }

    synthesize_with_daemon_retry(request).await
}

async fn synthesize_with_daemon_retry(request: SaySynthesisRequest<'_>) -> Result<()> {
    if crate::paths::find_models_dir().is_err() {
        if !request.quiet {
            println!("Voice models not found. Setting up VOICEVOX...");
        }
        ensure_models_available().await?;
    }

    let options = OwnedSynthesizeOptions { rate: request.rate };

    match DaemonClient::new_with_auto_start_at(&request.socket_path).await {
        Ok(mut client) => {
            let wav_data = client
                .synthesize(request.text, request.style_id, options)
                .await?;
            emit_synthesized_audio(&wav_data, request.output_file, request.quiet)?;
            Ok(())
        }
        Err(error) => {
            if !request.quiet {
                eprintln!("Failed to connect to daemon: {error}");
            }
            Err(error)
        }
    }
}
