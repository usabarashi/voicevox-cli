use anyhow::Result;
use std::path::Path;

use crate::app::AppOutput;
use crate::client::{ensure_models_available, DaemonClient};
use crate::synthesis::{synthesize_bytes, validate_basic_request, TextSynthesisRequest};

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

pub async fn connect_daemon_client_auto_start(socket_path: &Path) -> Result<DaemonClient> {
    DaemonClient::new_with_auto_start_at(socket_path).await
}

async fn ensure_models_on_demand(request: &DaemonSynthesisBytesRequest<'_>, output: &dyn AppOutput) -> Result<()> {
    if !request.ensure_models_if_missing {
        return Ok(());
    }

    if crate::paths::find_models_dir().is_err() {
        if !request.quiet_setup_messages {
            output.info("Voice models not found. Setting up VOICEVOX...");
        }
        ensure_models_available().await?;
    }

    Ok(())
}

pub async fn synthesize_bytes_via_daemon(
    request: &DaemonSynthesisBytesRequest<'_>,
    output: &dyn AppOutput,
) -> Result<Vec<u8>> {
    validate_text_synthesis_request(request.text, request.style_id, request.rate)?;
    ensure_models_on_demand(request, output).await?;

    let mut client = connect_daemon_client_auto_start(request.socket_path).await?;
    let synth_req = TextSynthesisRequest {
        text: request.text,
        style_id: request.style_id,
        rate: request.rate,
    };
    synthesize_bytes(&mut client, &synth_req).await
}
