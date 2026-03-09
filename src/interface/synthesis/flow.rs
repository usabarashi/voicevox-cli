use anyhow::{Result, anyhow};
use std::path::Path;
use tokio::sync::oneshot;

use crate::domain::synthesis::{TextSynthesisRequest, validate_basic_request};
use crate::infrastructure::daemon::client::DaemonClient;
use crate::interface::AppOutput;
use crate::interface::cli::download::{ensure_models_available, missing_startup_resources};
use crate::interface::synthesis::daemon::DaemonSynthesizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SynthesisPhase {
    Validate,
    EnsureResources,
    Connect,
    Synthesize,
}

// This lifecycle mirrors modeling/tla/Synthesis.tla at the same abstraction level:
// Idle -> Queued -> Synthesizing -> Done / Failed / Canceled.
// Rust currently has no explicit cancellation path in this flow, but we keep the
// state for model alignment and future cancellation integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SynthesisLifecycleState {
    Idle,
    Queued,
    Synthesizing,
    Done,
    Failed,
    Canceled,
}

impl SynthesisLifecycleState {
    #[must_use]
    const fn queue(self) -> Self {
        match self {
            Self::Idle => Self::Queued,
            _ => self,
        }
    }

    #[must_use]
    const fn start(self) -> Self {
        match self {
            Self::Queued => Self::Synthesizing,
            _ => self,
        }
    }

    #[must_use]
    const fn succeed(self) -> Self {
        match self {
            Self::Synthesizing => Self::Done,
            _ => self,
        }
    }

    #[must_use]
    const fn fail(self) -> Self {
        match self {
            Self::Idle | Self::Queued | Self::Synthesizing => Self::Failed,
            _ => self,
        }
    }

    #[must_use]
    const fn cancel(self) -> Self {
        match self {
            Self::Queued | Self::Synthesizing => Self::Canceled,
            _ => self,
        }
    }
}

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
    match synthesize_bytes_via_daemon_cancellable(request, output, None).await? {
        SynthesisFlowOutcome::Completed(wav_data) => Ok(wav_data),
        SynthesisFlowOutcome::Canceled(_reason) => Err(anyhow!(
            "Unexpected cancellation without cancellation receiver"
        )),
    }
}

pub enum SynthesisFlowOutcome {
    Completed(Vec<u8>),
    Canceled(String),
}

pub async fn synthesize_bytes_via_daemon_cancellable(
    request: &DaemonSynthesisBytesRequest<'_>,
    output: &dyn AppOutput,
    mut cancel_rx: Option<&mut oneshot::Receiver<String>>,
) -> Result<SynthesisFlowOutcome> {
    let mut phase = SynthesisPhase::Validate;
    let mut synthesizer: Option<DaemonSynthesizer> = None;
    let mut lifecycle = SynthesisLifecycleState::Idle.queue();

    loop {
        if matches!(phase, SynthesisPhase::Synthesize) {
            lifecycle = lifecycle.start();
        }

        if let Some(receiver) = cancel_rx.as_mut()
            && let Some(reason) = try_take_cancellation(receiver)
        {
            lifecycle = lifecycle.cancel();
            if matches!(lifecycle, SynthesisLifecycleState::Canceled) {
                return Ok(SynthesisFlowOutcome::Canceled(reason));
            }
        }

        let step_result = match cancel_rx.as_mut() {
            Some(receiver) => {
                tokio::select! {
                    reason = receiver => {
                        let reason = reason.unwrap_or_default();
                        lifecycle = lifecycle.cancel();
                        if matches!(lifecycle, SynthesisLifecycleState::Canceled) {
                            return Ok(SynthesisFlowOutcome::Canceled(reason));
                        }
                        Ok(SynthesisStep::Next(phase))
                    }
                    result = run_synthesis_phase(phase, request, output, &mut synthesizer) => result,
                }
            }
            None => run_synthesis_phase(phase, request, output, &mut synthesizer).await,
        };

        let step = match step_result {
            Ok(step) => step,
            Err(error) => {
                lifecycle = lifecycle.fail();
                debug_assert!(matches!(lifecycle, SynthesisLifecycleState::Failed));
                return Err(error);
            }
        };

        match step {
            SynthesisStep::Next(next) => phase = next,
            SynthesisStep::Done(wav_data) => {
                lifecycle = lifecycle.succeed();
                debug_assert!(matches!(lifecycle, SynthesisLifecycleState::Done));
                return Ok(SynthesisFlowOutcome::Completed(wav_data));
            }
        }
    }
}

fn try_take_cancellation(cancel_rx: &mut oneshot::Receiver<String>) -> Option<String> {
    match cancel_rx.try_recv() {
        Ok(reason) => Some(reason),
        Err(oneshot::error::TryRecvError::Closed) => Some(String::new()),
        Err(oneshot::error::TryRecvError::Empty) => None,
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
    synthesizer: &mut Option<DaemonSynthesizer>,
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
            let client = connect_daemon_client_auto_start(request.socket_path).await?;
            *synthesizer = Some(DaemonSynthesizer::new_with_client(client));
            Ok(SynthesisStep::Next(SynthesisPhase::Synthesize))
        }
        SynthesisPhase::Synthesize => {
            let mut synthesizer = synthesizer
                .take()
                .expect("synthesizer must exist in synthesize phase");
            let synth_req = TextSynthesisRequest {
                text: request.text,
                style_id: request.style_id,
                rate: request.rate,
            };
            let wav_data = synthesizer.synthesize_bytes(&synth_req).await?;
            Ok(SynthesisStep::Done(wav_data))
        }
    }
}
