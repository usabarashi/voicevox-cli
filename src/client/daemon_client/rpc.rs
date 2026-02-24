use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::ipc::{DaemonRequest, OwnedRequest, OwnedResponse, OwnedSynthesizeOptions};
use crate::voice::{format_speakers_output, Speaker};

use super::transport::{request_daemon_once, DAEMON_CONNECTION_TIMEOUT, DAEMON_RESPONSE_TIMEOUT};

fn daemon_response_error(context: &str, message: &str) -> anyhow::Error {
    anyhow!("{context}: {message}")
}

fn unexpected_daemon_response(context: &str) -> anyhow::Error {
    anyhow!("Unexpected response {context}")
}

async fn assert_compatible_daemon(socket_path: &Path, required_capability: &str) -> Result<()> {
    let response = request_daemon_once(
        socket_path,
        &OwnedRequest::GetServerInfo,
        DAEMON_CONNECTION_TIMEOUT,
        DAEMON_RESPONSE_TIMEOUT,
    )
    .await?;

    match response {
        OwnedResponse::ServerInfo {
            protocol_version,
            capabilities,
            ..
        } => {
            if protocol_version != crate::ipc::DAEMON_IPC_PROTOCOL_VERSION {
                return Err(anyhow!(
                    "Incompatible daemon IPC protocol version: expected {}, got {}",
                    crate::ipc::DAEMON_IPC_PROTOCOL_VERSION,
                    protocol_version
                ));
            }
            if !capabilities.iter().any(|cap| cap == required_capability) {
                return Err(anyhow!(
                    "Daemon does not advertise required capability: {required_capability}"
                ));
            }
            Ok(())
        }
        _ => Err(unexpected_daemon_response("during compatibility check")),
    }
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
    assert_compatible_daemon(socket_path, "synthesize").await?;
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
        OwnedResponse::Error { message } => Err(daemon_response_error("Daemon error", &message)),
        _ => Err(unexpected_daemon_response("from daemon")),
    }
}

/// Requests the speaker list from the daemon and prints it in CLI-friendly format.
///
/// # Errors
///
/// Returns an error if daemon connection, request/response serialization, or response
/// decoding fails, or if the daemon returns an error response.
pub async fn list_speakers_daemon(socket_path: &Path) -> Result<()> {
    assert_compatible_daemon(socket_path, "list_speakers").await?;
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
        OwnedResponse::Error { message } => Err(daemon_response_error("Daemon error", &message)),
        _ => Err(unexpected_daemon_response("from daemon")),
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
