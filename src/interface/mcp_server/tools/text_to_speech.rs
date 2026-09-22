use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use super::types::{ToolCallResult, success_result, text_result};
use crate::domain::synthesis::wav::concatenate_wav_segments;
use crate::domain::synthesis::{
    AttemptOutcome, RetryDecision, RetryPolicy, TextSynthesisRequest, validate_basic_request,
};
use crate::domain::text_to_speech::{
    SynthesizeParams, default_rate, default_streaming, validate_style_id,
};
use crate::infrastructure::daemon::startup;
use crate::interface::mcp_server::daemon_error::{
    format_daemon_client_error_for_mcp, is_retryable_daemon_synthesis_error,
};
use crate::interface::playback::{PlaybackOutcome, PlaybackRequest, emit_and_play};
use crate::interface::synthesis::flow::{
    DaemonSynthesisBytesRequest, NoopAppOutput, SynthesisFlowOutcome,
    synthesize_bytes_via_daemon_cancellable,
};
use crate::interface::synthesis::mode::{SynthesisMode, select_synthesis_mode_with_config};

const MCP_DAEMON_MAX_RETRIES: u32 = 2;

#[derive(Debug, Deserialize)]
struct TextToSpeechToolInput {
    text: String,
    style_id: u32,
    #[serde(default = "default_rate")]
    rate: f32,
    #[serde(default = "default_streaming")]
    streaming: bool,
}

/// Result of one client-side synthesis attempt, as seen by the retry loop.
enum AttemptCallOutcome {
    Completed(Vec<u8>),
    Cancelled(String),
    Failed(anyhow::Error),
}

/// Result of waiting for a backoff.
enum WaitOutcome {
    Elapsed,
    Cancelled(String),
}

/// One client-side synthesis attempt (the environment boundary of the loop).
///
/// The daemon is an environment: an attempt either produces bytes, is canceled,
/// or fails with an error whose retryability is classified outside this trait.
trait SynthesisAttempt {
    async fn run(
        &mut self,
        request: &DaemonSynthesisBytesRequest<'_>,
        cancel_rx: Option<&mut oneshot::Receiver<String>>,
    ) -> AttemptCallOutcome;
}

/// Waits for a backoff delay. Cancellation has priority when both are ready.
trait BackoffWaiter {
    async fn wait(
        &mut self,
        delay: Duration,
        cancel_rx: Option<&mut oneshot::Receiver<String>>,
    ) -> WaitOutcome;
}

/// Outcome of the retry loop, including the observed attempt/backoff counts.
#[derive(Default)]
struct RetryLoopResult {
    wav_data: Option<Vec<u8>>,
    last_error: Option<anyhow::Error>,
    cancellation: Option<String>,
    attempts_started: u32,
    backoffs_started: u32,
}

/// Executes the `text_to_speech` tool without external cancellation.
///
/// # Errors
///
/// Returns an error if parameter validation or synthesis fails.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech(arguments: Value) -> Result<ToolCallResult> {
    handle_text_to_speech_cancellable(arguments, None).await
}

/// Executes the `text_to_speech` tool with optional cancellation support.
///
/// # Errors
///
/// Returns an error if parameters are invalid or synthesis fails.
#[allow(clippy::future_not_send)]
pub async fn handle_text_to_speech_cancellable(
    arguments: Value,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let parsed: TextToSpeechToolInput =
        serde_json::from_value(arguments).context("Invalid parameters for text_to_speech")?;
    validate_style_id(parsed.style_id)?;
    let params = SynthesizeParams {
        text: parsed.text,
        style_id: parsed.style_id,
        rate: parsed.rate,
        streaming: parsed.streaming,
    };
    validate_basic_request(&TextSynthesisRequest {
        text: &params.text,
        style_id: params.style_id,
        rate: params.rate,
    })?;

    if params.streaming {
        handle_streaming_synthesis(params, cancel_rx).await
    } else {
        handle_daemon_synthesis(params, cancel_rx).await
    }
}

/// Runs a potentially non-Send text-to-speech async task on a blocking worker thread.
pub fn spawn_non_send_text_to_speech_task<F>(future_factory: F)
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> + Send + 'static,
{
    let runtime_handle = Handle::current();
    tokio::task::spawn_blocking(move || {
        runtime_handle.block_on(future_factory());
    });
}

#[allow(clippy::future_not_send)]
async fn handle_streaming_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;
    let synthesis = do_streaming_synthesis(&text, style_id, rate);

    if let Some(mut cancel_rx) = cancel_rx {
        if let Some(reason) = try_take_cancellation(&mut cancel_rx) {
            return Ok(cancellation_result(reason));
        }
        let wav_data = tokio::select! {
            result = synthesis => result,
            reason = &mut cancel_rx => {
                return Ok(cancellation_result(reason.unwrap_or_default()));
            }
        }?;
        if let Some(cancelled_result) = play_generated_audio(&wav_data, Some(cancel_rx)).await? {
            return Ok(cancelled_result);
        }
        Ok(success_result())
    } else {
        let wav_data = synthesis.await?;
        play_generated_audio(&wav_data, None).await?;
        Ok(success_result())
    }
}

#[allow(clippy::future_not_send)]
async fn do_streaming_synthesis(text: &str, style_id: u32, rate: f32) -> Result<Vec<u8>> {
    let config = crate::config::Config::default();
    let mut synthesizer = match select_synthesis_mode_with_config(true, &config).await {
        Ok(SynthesisMode::Streaming(synthesizer)) => synthesizer,
        Ok(SynthesisMode::Daemon(_)) => unreachable!(),
        Err(error) => return Err(error.context("Failed to create streaming synthesizer")),
    };

    let request = TextSynthesisRequest {
        text,
        style_id,
        rate,
    };
    let wav_segments = synthesizer
        .request_streaming_synthesis_segments(request.text, request.style_id, request.rate)
        .await
        .context("Streaming synthesis failed")?;

    let wav_data =
        concatenate_wav_segments(&wav_segments).context("Failed to concatenate WAV segments")?;

    Ok(wav_data)
}

#[allow(clippy::future_not_send)]
async fn handle_daemon_synthesis(
    params: SynthesizeParams,
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<ToolCallResult> {
    let SynthesizeParams {
        text,
        style_id,
        rate,
        streaming: _,
    } = params;

    let socket_path = crate::infrastructure::paths::get_socket_path();
    let request = DaemonSynthesisBytesRequest {
        text: &text,
        style_id,
        rate,
        socket_path: &socket_path,
        ensure_models_if_missing: false,
        quiet_setup_messages: true,
    };

    let policy = RetryPolicy::new(MCP_DAEMON_MAX_RETRIES);
    let mut cancel_rx = cancel_rx;
    let result = run_retry_loop(
        policy,
        &mut RealSynthesisAttempt,
        &mut RealBackoffWaiter,
        &request,
        &mut cancel_rx,
    )
    .await;

    if let Some(reason) = result.cancellation {
        return Ok(cancellation_result(reason));
    }

    let Some(wav_data) = result.wav_data else {
        let error = result
            .last_error
            .expect("last error should exist when synthesis failed");
        return Ok(text_result(
            format_daemon_client_error_for_mcp(&error),
            true,
        ));
    };

    if let Some(cancelled_result) = play_generated_audio(&wav_data, cancel_rx).await? {
        return Ok(cancelled_result);
    }

    Ok(success_result())
}

/// Drives the client-side synthesis retry loop.
///
/// The retry/backoff decision comes from `domain::synthesis::retry`; this loop
/// only orchestrates. Cancellation is evaluated before every attempt and has
/// priority while waiting for a backoff. Attempts and backoffs are counted so
/// the observed behavior can be asserted.
#[allow(clippy::future_not_send)]
async fn run_retry_loop<A, W>(
    policy: RetryPolicy,
    attempt: &mut A,
    waiter: &mut W,
    request: &DaemonSynthesisBytesRequest<'_>,
    cancel_rx: &mut Option<oneshot::Receiver<String>>,
) -> RetryLoopResult
where
    A: SynthesisAttempt,
    W: BackoffWaiter,
{
    let mut result = RetryLoopResult::default();
    let mut retry_index: u32 = 0;

    loop {
        if let Some(reason) = take_cancellation(cancel_rx) {
            result.cancellation = Some(reason);
            return result;
        }

        result.attempts_started += 1;
        match attempt.run(request, cancel_rx.as_mut()).await {
            AttemptCallOutcome::Completed(wav_data) => {
                result.wav_data = Some(wav_data);
                return result;
            }
            AttemptCallOutcome::Cancelled(reason) => {
                result.cancellation = Some(reason);
                return result;
            }
            AttemptCallOutcome::Failed(error) => {
                let retryable = is_retryable_daemon_synthesis_error(&error);
                result.last_error = Some(error);
                let outcome = if retryable {
                    AttemptOutcome::RetryableFailure
                } else {
                    AttemptOutcome::FatalFailure
                };
                if policy.after_attempt(result.attempts_started, outcome) == RetryDecision::Finish {
                    return result;
                }

                result.backoffs_started += 1;
                let delay = policy.backoff_delay(
                    retry_index,
                    startup::initial_retry_delay(),
                    startup::max_retry_delay(),
                );
                retry_index += 1;
                match waiter.wait(delay, cancel_rx.as_mut()).await {
                    WaitOutcome::Elapsed => {}
                    WaitOutcome::Cancelled(reason) => {
                        result.cancellation = Some(reason);
                        return result;
                    }
                }
            }
        }
    }
}

fn take_cancellation(cancel_rx: &mut Option<oneshot::Receiver<String>>) -> Option<String> {
    cancel_rx.as_mut().and_then(try_take_cancellation)
}

/// Production synthesis attempt: delegates to the daemon client.
struct RealSynthesisAttempt;

impl SynthesisAttempt for RealSynthesisAttempt {
    async fn run(
        &mut self,
        request: &DaemonSynthesisBytesRequest<'_>,
        cancel_rx: Option<&mut oneshot::Receiver<String>>,
    ) -> AttemptCallOutcome {
        match synthesize_bytes_via_daemon_cancellable(request, &NoopAppOutput, cancel_rx).await {
            Ok(SynthesisFlowOutcome::Completed(wav_data)) => AttemptCallOutcome::Completed(wav_data),
            Ok(SynthesisFlowOutcome::Canceled(reason)) => AttemptCallOutcome::Cancelled(reason),
            Err(error) => AttemptCallOutcome::Failed(error),
        }
    }
}

/// Production backoff wait: real timer, cancellation first (`biased`).
struct RealBackoffWaiter;

impl BackoffWaiter for RealBackoffWaiter {
    async fn wait(
        &mut self,
        delay: Duration,
        cancel_rx: Option<&mut oneshot::Receiver<String>>,
    ) -> WaitOutcome {
        match cancel_rx {
            Some(receiver) => {
                tokio::select! {
                    biased;
                    reason = receiver => WaitOutcome::Cancelled(reason.unwrap_or_default()),
                    () = tokio::time::sleep(delay) => WaitOutcome::Elapsed,
                }
            }
            None => {
                tokio::time::sleep(delay).await;
                WaitOutcome::Elapsed
            }
        }
    }
}

fn cancellation_message(reason: &str) -> String {
    if reason.is_empty() {
        "Synthesis cancelled".to_string()
    } else {
        format!("Synthesis cancelled: {reason}")
    }
}

fn cancellation_result(reason: String) -> ToolCallResult {
    text_result(cancellation_message(&reason), true)
}

fn try_take_cancellation(cancel_rx: &mut oneshot::Receiver<String>) -> Option<String> {
    match cancel_rx.try_recv() {
        Ok(reason) => Some(reason),
        Err(oneshot::error::TryRecvError::Closed) => Some(String::new()),
        Err(oneshot::error::TryRecvError::Empty) => None,
    }
}

#[allow(clippy::future_not_send)]
async fn play_generated_audio(
    wav_data: &[u8],
    cancel_rx: Option<oneshot::Receiver<String>>,
) -> Result<Option<ToolCallResult>> {
    match emit_and_play(PlaybackRequest {
        wav_data,
        output_file: None,
        play: true,
        cancel_rx,
    })
    .await
    .context("Failed to play synthesized audio")?
    {
        PlaybackOutcome::Completed => Ok(None),
        PlaybackOutcome::Cancelled(reason) => Ok(Some(cancellation_result(reason))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::daemon::client::daemon_response_error;
    use crate::infrastructure::ipc::DaemonErrorCode;
    use crate::interface::mcp_server::tools::types::ToolContent;
    use serde_json::json;
    use tokio::sync::oneshot;

    #[test]
    fn retryable_policy_matches_daemon_error_code() {
        let invalid = daemon_response_error("ctx", DaemonErrorCode::InvalidTargetId, "bad id");
        assert!(!is_retryable_daemon_synthesis_error(&invalid));

        let model_load =
            daemon_response_error("ctx", DaemonErrorCode::ModelLoadFailed, "model missing");
        assert!(!is_retryable_daemon_synthesis_error(&model_load));

        let synthesis =
            daemon_response_error("ctx", DaemonErrorCode::SynthesisFailed, "temporary failure");
        assert!(is_retryable_daemon_synthesis_error(&synthesis));

        let internal = daemon_response_error("ctx", DaemonErrorCode::Internal, "daemon panic");
        assert!(is_retryable_daemon_synthesis_error(&internal));
    }

    #[tokio::test]
    async fn cancellation_signal_short_circuits_daemon_synthesis() {
        let args = json!({
            "text": "テスト",
            "style_id": 3,
            "streaming": false,
        });
        let (cancel_tx, cancel_rx) = oneshot::channel::<String>();
        let _ = cancel_tx.send("ESC pressed".to_string());

        let result = handle_text_to_speech_cancellable(args, Some(cancel_rx))
            .await
            .expect("cancellation should return tool result");

        assert_eq!(result.is_error, Some(true));
        let Some(ToolContent::Text { text }) = result.content.first() else {
            panic!("expected text content in cancellation result");
        };
        assert!(text.contains("cancelled"));
        assert!(text.contains("ESC pressed"));
    }

    // ---- Retry-loop orchestration tests ---------------------------------

    fn retryable_error() -> anyhow::Error {
        daemon_response_error("ctx", DaemonErrorCode::SynthesisFailed, "temporary failure")
    }

    fn fatal_error() -> anyhow::Error {
        daemon_response_error("ctx", DaemonErrorCode::InvalidTargetId, "bad id")
    }

    fn test_request(socket_path: &std::path::Path) -> DaemonSynthesisBytesRequest<'_> {
        DaemonSynthesisBytesRequest {
            text: "test",
            style_id: 3,
            rate: 1.0,
            socket_path,
            ensure_models_if_missing: false,
            quiet_setup_messages: true,
        }
    }

    /// Scripted synthesis attempt. Counts how many attempts the production loop
    /// actually started.
    #[derive(Default)]
    struct FakeAttempt {
        script: std::collections::VecDeque<AttemptCallOutcome>,
        calls: u32,
    }

    impl FakeAttempt {
        fn new(script: Vec<AttemptCallOutcome>) -> Self {
            Self {
                script: script.into(),
                calls: 0,
            }
        }
    }

    impl SynthesisAttempt for FakeAttempt {
        async fn run(
            &mut self,
            _request: &DaemonSynthesisBytesRequest<'_>,
            _cancel_rx: Option<&mut oneshot::Receiver<String>>,
        ) -> AttemptCallOutcome {
            self.calls += 1;
            self.script.pop_front().unwrap_or_else(|| {
                AttemptCallOutcome::Failed(anyhow::anyhow!("unexpected attempt"))
            })
        }
    }

    /// Controllable wait seam. Returning `Cancelled` models cancellation
    /// arriving while the timer is still pending, without relying on wall-clock
    /// or auto-advancing time.
    #[derive(Default)]
    struct FakeWaiter {
        script: std::collections::VecDeque<WaitOutcome>,
        waits: u32,
    }

    impl FakeWaiter {
        fn new(script: Vec<WaitOutcome>) -> Self {
            Self {
                script: script.into(),
                waits: 0,
            }
        }
    }

    impl BackoffWaiter for FakeWaiter {
        async fn wait(
            &mut self,
            _delay: Duration,
            _cancel_rx: Option<&mut oneshot::Receiver<String>>,
        ) -> WaitOutcome {
            self.waits += 1;
            self.script.pop_front().unwrap_or(WaitOutcome::Elapsed)
        }
    }

    #[tokio::test]
    async fn retry_loop_exhausts_retryable_failures() {
        let policy = RetryPolicy::new(2);
        let mut attempt = FakeAttempt::new(vec![
            AttemptCallOutcome::Failed(retryable_error()),
            AttemptCallOutcome::Failed(retryable_error()),
            AttemptCallOutcome::Failed(retryable_error()),
        ]);
        let mut waiter = FakeWaiter::new(vec![]);
        let socket = std::path::Path::new("/tmp/does-not-exist.sock");
        let request = test_request(socket);
        let mut cancel_rx = None;

        let result =
            run_retry_loop(policy, &mut attempt, &mut waiter, &request, &mut cancel_rx).await;

        assert_eq!(result.attempts_started, 3);
        assert_eq!(result.backoffs_started, 2);
        assert_eq!(attempt.calls, 3);
        assert_eq!(waiter.waits, 2);
        assert!(result.wav_data.is_none());
        assert!(result.last_error.is_some());
        assert!(result.cancellation.is_none());
    }

    #[tokio::test]
    async fn retry_loop_succeeds_after_one_retry() {
        let policy = RetryPolicy::new(2);
        let mut attempt = FakeAttempt::new(vec![
            AttemptCallOutcome::Failed(retryable_error()),
            AttemptCallOutcome::Completed(vec![1, 2, 3]),
        ]);
        let mut waiter = FakeWaiter::new(vec![]);
        let socket = std::path::Path::new("/tmp/does-not-exist.sock");
        let request = test_request(socket);
        let mut cancel_rx = None;

        let result =
            run_retry_loop(policy, &mut attempt, &mut waiter, &request, &mut cancel_rx).await;

        assert_eq!(result.attempts_started, 2);
        assert_eq!(result.backoffs_started, 1);
        assert_eq!(attempt.calls, 2);
        assert_eq!(result.wav_data, Some(vec![1, 2, 3]));
        assert!(result.cancellation.is_none());
    }

    #[tokio::test]
    async fn retry_loop_stops_immediately_on_fatal_error() {
        let policy = RetryPolicy::new(2);
        let mut attempt = FakeAttempt::new(vec![AttemptCallOutcome::Failed(fatal_error())]);
        let mut waiter = FakeWaiter::new(vec![]);
        let socket = std::path::Path::new("/tmp/does-not-exist.sock");
        let request = test_request(socket);
        let mut cancel_rx = None;

        let result =
            run_retry_loop(policy, &mut attempt, &mut waiter, &request, &mut cancel_rx).await;

        assert_eq!(result.attempts_started, 1);
        assert_eq!(result.backoffs_started, 0);
        assert_eq!(attempt.calls, 1);
        assert_eq!(waiter.waits, 0);
        assert!(result.last_error.is_some());
    }

    #[tokio::test]
    async fn retry_loop_cancel_during_backoff_prevents_next_attempt() {
        let policy = RetryPolicy::new(2);
        let mut attempt = FakeAttempt::new(vec![AttemptCallOutcome::Failed(retryable_error())]);
        let mut waiter = FakeWaiter::new(vec![WaitOutcome::Cancelled("ESC pressed".to_string())]);
        let socket = std::path::Path::new("/tmp/does-not-exist.sock");
        let request = test_request(socket);
        let mut cancel_rx = None;

        let result =
            run_retry_loop(policy, &mut attempt, &mut waiter, &request, &mut cancel_rx).await;

        assert_eq!(result.attempts_started, 1);
        assert_eq!(result.backoffs_started, 1);
        assert_eq!(attempt.calls, 1, "no attempt may start after cancellation");
        assert_eq!(result.cancellation.as_deref(), Some("ESC pressed"));
        assert!(result.wav_data.is_none());
    }

    #[tokio::test]
    async fn retry_loop_cancel_before_first_attempt_starts_nothing() {
        let policy = RetryPolicy::new(2);
        let mut attempt = FakeAttempt::new(vec![]);
        let mut waiter = FakeWaiter::new(vec![]);
        let socket = std::path::Path::new("/tmp/does-not-exist.sock");
        let request = test_request(socket);
        let (cancel_tx, cancel_rx) = oneshot::channel::<String>();
        let _ = cancel_tx.send("ESC pressed".to_string());
        let mut cancel_rx = Some(cancel_rx);

        let result =
            run_retry_loop(policy, &mut attempt, &mut waiter, &request, &mut cancel_rx).await;

        assert_eq!(result.attempts_started, 0);
        assert_eq!(attempt.calls, 0);
        assert_eq!(result.cancellation.as_deref(), Some("ESC pressed"));
    }

    #[tokio::test]
    async fn real_waiter_prefers_cancellation_over_the_timer() {
        let (cancel_tx, cancel_rx) = oneshot::channel::<String>();
        let _ = cancel_tx.send("ESC pressed".to_string());
        let mut cancel_rx = Some(cancel_rx);

        let outcome = RealBackoffWaiter
            .wait(Duration::from_secs(3600), cancel_rx.as_mut())
            .await;

        assert!(matches!(outcome, WaitOutcome::Cancelled(_)));
    }
}
