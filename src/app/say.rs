use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::app::{AppOutput, StdAppOutput};
use crate::client::{emit_synthesized_audio, ensure_models_available, DaemonClient};
use crate::synthesis::{synthesize_bytes, validate_basic_request, TextSynthesisRequest};

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
    let output = StdAppOutput;
    run_say_synthesis_with_output(request, &output).await
}

pub async fn run_say_synthesis_with_output(
    request: SaySynthesisRequest<'_>,
    output: &dyn AppOutput,
) -> Result<()> {
    validate_basic_request(&TextSynthesisRequest {
        text: request.text,
        style_id: request.style_id,
        rate: request.rate,
    })?;

    synthesize_with_daemon_retry(request, output).await
}

async fn synthesize_with_daemon_retry(
    request: SaySynthesisRequest<'_>,
    output: &dyn AppOutput,
) -> Result<()> {
    if crate::paths::find_models_dir().is_err() {
        if !request.quiet {
            output.info("Voice models not found. Setting up VOICEVOX...");
        }
        ensure_models_available().await?;
    }

    match DaemonClient::new_with_auto_start_at(&request.socket_path).await {
        Ok(mut client) => {
            let synth_req = TextSynthesisRequest {
                text: request.text,
                style_id: request.style_id,
                rate: request.rate,
            };
            let wav_data = synthesize_bytes(&mut client, &synth_req).await?;
            emit_synthesized_audio(&wav_data, request.output_file, request.quiet)?;
            Ok(())
        }
        Err(error) => {
            if !request.quiet {
                output.error(&format!("Failed to connect to daemon: {error}"));
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::output::BufferAppOutput;

    #[tokio::test]
    async fn rejects_empty_text_before_side_effects() {
        let output = BufferAppOutput::default();
        let request = SaySynthesisRequest {
            text: "   ",
            style_id: 1,
            rate: 1.0,
            output_file: None,
            quiet: true,
            socket_path: PathBuf::from("/tmp/unused.sock"),
        };

        let error = run_say_synthesis_with_output(request, &output)
            .await
            .expect_err("expected validation error");

        assert!(error
            .to_string()
            .contains("No text provided. Use command line argument"));
        assert!(output.infos().is_empty());
        assert!(output.errors().is_empty());
    }
}
