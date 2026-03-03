use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::app::{
    synthesize_bytes_via_daemon, validate_text_synthesis_request, AppOutput,
    DaemonSynthesisBytesRequest, StdAppOutput,
};
use crate::app::system_state::SayPhase;
use crate::client::{emit_synthesized_audio, format_daemon_rpc_error_for_cli};

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
    let mut phase = SayPhase::Validate;
    let mut wav_data: Option<Vec<u8>> = None;

    loop {
        match run_say_phase(phase, &request, output, &mut wav_data).await? {
            SayStep::Next(next) => phase = next,
            SayStep::Done => return Ok(()),
        }
    }
}

enum SayStep {
    Next(SayPhase),
    Done,
}

async fn run_say_phase(
    phase: SayPhase,
    request: &SaySynthesisRequest<'_>,
    output: &dyn AppOutput,
    wav_data: &mut Option<Vec<u8>>,
) -> Result<SayStep> {
    match phase {
        SayPhase::Validate => {
            validate_text_synthesis_request(request.text, request.style_id, request.rate)?;
            Ok(SayStep::Next(SayPhase::Synthesize))
        }
        SayPhase::Synthesize => {
            let synth_request = DaemonSynthesisBytesRequest {
                text: request.text,
                style_id: request.style_id,
                rate: request.rate,
                socket_path: &request.socket_path,
                ensure_models_if_missing: true,
                quiet_setup_messages: request.quiet,
            };

            match synthesize_bytes_via_daemon(&synth_request, output).await {
                Ok(data) => {
                    *wav_data = Some(data);
                    Ok(SayStep::Next(SayPhase::Emit))
                }
                Err(error) => {
                    if !request.quiet {
                        output.error(&format_daemon_rpc_error_for_cli(&error));
                    }
                    Err(error)
                }
            }
        }
        SayPhase::Emit => {
            let wav_data = wav_data
                .take()
                .expect("wav_data must be present in emit phase");
            emit_synthesized_audio(&wav_data, request.output_file, request.quiet)?;
            Ok(SayStep::Done)
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
