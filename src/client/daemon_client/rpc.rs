use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse, OwnedSynthesizeOptions};
use crate::voice::{format_speakers_output, Speaker};

use super::daemon_response_error;
use super::transport::{request_daemon_once, DAEMON_CONNECTION_TIMEOUT, DAEMON_RESPONSE_TIMEOUT};

fn unexpected_daemon_response(operation: &str, expected: &str) -> anyhow::Error {
    anyhow!("Daemon returned an unexpected response while {operation} (expected: {expected})")
}

/// Sends a synthesis request to an already running daemon and handles output/playback.
///
/// # Errors
///
/// Returns an error if daemon connection/setup fails, request/response framing fails,
/// synthesis returns an error response, file writing fails, or audio playback fails.
pub async fn daemon_mode(
    text: &str,
    style_id: u32,
    options: OwnedSynthesizeOptions,
    output_file: Option<&Path>,
    quiet: bool,
    socket_path: &Path,
) -> Result<()> {
    let request = OwnedRequest::Synthesize {
        text: text.to_string(),
        style_id,
        options,
    };
    let response = request_daemon_once(
        socket_path,
        &request,
        DAEMON_CONNECTION_TIMEOUT,
        DAEMON_RESPONSE_TIMEOUT,
    )
    .await?;

    match response {
        OwnedResponse::SynthesizeResult { wav_data } => {
            crate::client::audio::emit_synthesized_audio(&wav_data, output_file, quiet)?;
            Ok(())
        }
        OwnedResponse::Error { code, message } => {
            Err(daemon_response_error("Daemon error", code, &message))
        }
        _ => Err(unexpected_daemon_response(
            "handling synthesize request",
            "SynthesizeResult or Error",
        )),
    }
}

/// Requests the speaker list from the daemon and prints it in CLI-friendly format.
///
/// # Errors
///
/// Returns an error if daemon connection, request/response serialization, or response
/// decoding fails, or if the daemon returns an error response.
pub async fn list_speakers_daemon(socket_path: &Path) -> Result<()> {
    let response = request_daemon_once(
        socket_path,
        &DaemonRequest::ListSpeakers,
        DAEMON_CONNECTION_TIMEOUT,
        DAEMON_RESPONSE_TIMEOUT,
    )
    .await?;

    match response {
        OwnedResponse::SpeakersListWithModels {
            speakers,
            style_to_model,
        } => {
            print_speakers(&speakers, Some(&style_to_model));
            Ok(())
        }
        OwnedResponse::Error { code, message } => {
            Err(daemon_response_error("Daemon error", code, &message))
        }
        _ => Err(unexpected_daemon_response(
            "listing speakers",
            "SpeakersListWithModels or Error",
        )),
    }
}

fn print_speakers(speakers: &[Speaker], style_to_model: Option<&HashMap<u32, u32>>) {
    println!(
        "{}",
        format_speakers_output(
            "All available speakers and styles from daemon:",
            speakers,
            style_to_model
        )
    );
}
