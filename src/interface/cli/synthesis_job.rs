use anyhow::Result;
use std::path::Path;

use crate::domain::synthesis::{validate_basic_request, TextSynthesisRequest};
use crate::domain::workflow_state::SynthesisPhase;
use crate::interface::cli::synthesis_client::synthesize_bytes;
use crate::interface::cli::{ensure_models_available, missing_startup_resources, DaemonRpcClient};
use crate::interface::AppOutput;

#[derive(Default, Clone, Copy)]
pub struct NoopAppOutput;

impl AppOutput for NoopAppOutput {
    fn info(&self, _message: &str) {}
    fn error(&self, _message: &str) {}
}

pub struct DaemonSynthesisBytesRequest<'a> {
    pub text: &'a str,
    pub style_id: u32,
    pub rate: f32,
    pub socket_path: &'a Path,
    pub ensure_models_if_missing: bool,
    pub quiet_setup_messages: bool,
}

pub fn validate_text_synthesis_request(text: &str, style_id: u32, rate: f32) -> Result<()> {
    validate_basic_request(&TextSynthesisRequest {
        text,
        style_id,
        rate,
    })
}

pub async fn connect_daemon_rpc_auto_start(socket_path: &Path) -> Result<DaemonRpcClient> {
    DaemonRpcClient::new_with_auto_start_at(socket_path).await
}

async fn ensure_models_on_demand(
    request: &DaemonSynthesisBytesRequest<'_>,
    output: &dyn AppOutput,
) -> Result<()> {
    if !request.ensure_models_if_missing {
        return Ok(());
    }

    let missing = missing_startup_resources();
    if !missing.is_empty() {
        if !request.quiet_setup_messages {
            output.info(&format!(
                "VOICEVOX resources not found ({}). Setting up VOICEVOX...",
                missing.join(", ")
            ));
        }
        ensure_models_available().await?;
    }

    Ok(())
}

pub async fn synthesize_bytes_via_daemon(
    request: &DaemonSynthesisBytesRequest<'_>,
    output: &dyn AppOutput,
) -> Result<Vec<u8>> {
    let mut phase = SynthesisPhase::Validate;
    let mut client: Option<DaemonRpcClient> = None;

    loop {
        match run_synthesis_phase(phase, request, output, &mut client).await? {
            SynthesisStep::Next(next) => phase = next,
            SynthesisStep::Done(wav_data) => return Ok(wav_data),
        }
    }
}

enum SynthesisStep {
    Next(SynthesisPhase),
    Done(Vec<u8>),
}

async fn run_synthesis_phase(
    phase: SynthesisPhase,
    request: &DaemonSynthesisBytesRequest<'_>,
    output: &dyn AppOutput,
    client: &mut Option<DaemonRpcClient>,
) -> Result<SynthesisStep> {
    match phase {
        SynthesisPhase::Validate => {
            validate_text_synthesis_request(request.text, request.style_id, request.rate)?;
            Ok(SynthesisStep::Next(SynthesisPhase::EnsureResources))
        }
        SynthesisPhase::EnsureResources => {
            ensure_models_on_demand(request, output).await?;
            Ok(SynthesisStep::Next(SynthesisPhase::Connect))
        }
        SynthesisPhase::Connect => {
            *client = Some(connect_daemon_rpc_auto_start(request.socket_path).await?);
            Ok(SynthesisStep::Next(SynthesisPhase::Synthesize))
        }
        SynthesisPhase::Synthesize => {
            let mut client = client
                .take()
                .expect("client must exist in synthesize phase");
            let synth_req = TextSynthesisRequest {
                text: request.text,
                style_id: request.style_id,
                rate: request.rate,
            };
            let wav_data = synthesize_bytes(&mut client, &synth_req).await?;
            Ok(SynthesisStep::Done(wav_data))
        }
    }
}
