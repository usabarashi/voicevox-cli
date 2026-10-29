use anyhow::Result;
use futures_util::StreamExt;
use serde_json::Value;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio_util::codec::{FramedRead, LinesCodec};

use crate::interface::mcp_server::protocol::{
    CacheScope, DiscoverResult, INVALID_PARAMS, INVALID_REQUEST, JsonRpcResponse, ListToolsResult,
    METHOD_NOT_FOUND, NotificationMethod, PARSE_ERROR, RESULT_TYPE_COMPLETE, RequestMethod,
    ResultMeta, STATIC_RESULT_TTL_MS, SUPPORTED_VERSIONS, ServerCapabilities,
    parse_notification_message, parse_request_message, request_id_key, serialize_success_response,
};
use crate::interface::mcp_server::server::runtime::ActiveRequests;
use crate::interface::mcp_server::tools::registry::{is_known_tool, tool_definitions};

const RESPONSE_QUEUE_CAPACITY: usize = 64;
const MAX_JSONRPC_LINE_BYTES: usize = 256 * 1024;

pub async fn run_stdio_server() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let codec = LinesCodec::new_with_max_length(MAX_JSONRPC_LINE_BYTES);
    let mut lines = FramedRead::new(stdin, codec);

    let (response_tx, mut response_rx) = mpsc::channel::<JsonRpcResponse>(RESPONSE_QUEUE_CAPACITY);
    let active_requests = ActiveRequests::new(response_tx);

    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            frame = lines.next() => {
                let (control, cancelled) =
                    process_frame(frame, &active_requests, &mut stdout).await?;
                if let Some(request_id) = cancelled {
                    for response in take_queued_responses(&mut response_rx, &request_id) {
                        send_response(&response, &mut stdout).await?;
                    }
                }
                match control {
                    LoopControl::Continue => {}
                    LoopControl::Break => {
                        active_requests.cancel_all_requests("Client disconnected").await;
                        break;
                    }
                }
            }
            Some(response) = response_rx.recv() => {
                if send_response(&response, &mut stdout).await.is_err() {
                    active_requests.cancel_all_requests("Failed to write response").await;
                    break;
                }
            }
            _ = &mut shutdown => {
                active_requests.cancel_all_requests("Server shutdown").await;
                break;
            }
        }
    }

    Ok(())
}

enum LoopControl {
    Continue,
    Break,
}

async fn process_frame(
    frame: Option<Result<String, tokio_util::codec::LinesCodecError>>,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) -> Result<(LoopControl, Option<String>)> {
    let line = match frame {
        None => return Ok((LoopControl::Break, None)),
        Some(Err(tokio_util::codec::LinesCodecError::MaxLineLengthExceeded)) => {
            let response =
                JsonRpcResponse::error(Value::Null, INVALID_REQUEST, "Request too large");
            send_response(&response, stdout).await?;
            return Ok((LoopControl::Continue, None));
        }
        Some(Err(e)) => return Err(e.into()),
        Some(Ok(line)) if line.trim().is_empty() => return Ok((LoopControl::Continue, None)),
        Some(Ok(line)) => line,
    };

    let Some(raw_message) = parse_json_request(&line, stdout).await? else {
        return Ok((LoopControl::Continue, None));
    };

    if raw_message.get("id").is_none() {
        let cancelled = handle_notification(raw_message, active_requests).await;
        return Ok((LoopControl::Continue, cancelled));
    }

    handle_request(raw_message, active_requests, stdout).await?;
    Ok((LoopControl::Continue, None))
}

async fn parse_json_request(line: &str, stdout: &mut tokio::io::Stdout) -> Result<Option<Value>> {
    match serde_json::from_str(line) {
        Ok(request) => Ok(Some(request)),
        Err(_) => {
            let error_response = JsonRpcResponse::error(Value::Null, PARSE_ERROR, "Parse error");
            send_response(&error_response, stdout).await?;
            Ok(None)
        }
    }
}

async fn handle_request(
    raw_message: Value,
    active_requests: &ActiveRequests,
    stdout: &mut tokio::io::Stdout,
) -> Result<()> {
    let response_id = raw_message.get("id").cloned().unwrap_or(Value::Null);
    let request = match parse_request_message(raw_message) {
        Ok(request) => request,
        Err(parse_error) => {
            let error_response = parse_error.into_response(response_id);
            send_response(&error_response, stdout).await?;
            return Ok(());
        }
    };

    match request.method {
        RequestMethod::ServerDiscover => {
            let result = DiscoverResult {
                result_type: RESULT_TYPE_COMPLETE.to_string(),
                supported_versions: SUPPORTED_VERSIONS.map(str::to_string).to_vec(),
                capabilities: server_capabilities(),
                instructions: crate::infrastructure::mcp_instructions::load_mcp_instructions(),
                ttl_ms: STATIC_RESULT_TTL_MS,
                cache_scope: CacheScope::Public,
                meta: ResultMeta::default(),
            };
            let response = serialize_success_response(request.id, result);
            send_response(&response, stdout).await?;
        }
        RequestMethod::ToolsList => {
            let result = ListToolsResult {
                result_type: RESULT_TYPE_COMPLETE.to_string(),
                tools: tool_definitions(),
                ttl_ms: STATIC_RESULT_TTL_MS,
                cache_scope: CacheScope::Public,
                meta: ResultMeta::default(),
            };
            let response = serialize_success_response(request.id, result);
            send_response(&response, stdout).await?;
        }
        RequestMethod::ToolsCall(call) => {
            // An unknown tool is a protocol error, not a tool result: no argument
            // change lets the model recover from a name that does not exist.
            if !is_known_tool(&call.name) {
                let response = JsonRpcResponse::error(
                    request.id,
                    INVALID_PARAMS,
                    format!("Unknown tool: {}", call.name),
                );
                send_response(&response, stdout).await?;
                return Ok(());
            }

            let Some(request_id) = request_id_key(&request.id) else {
                let response = JsonRpcResponse::error(
                    request.id,
                    INVALID_REQUEST,
                    "Request id must be a string or number",
                );
                send_response(&response, stdout).await?;
                return Ok(());
            };
            active_requests
                .spawn_tool_handler(request_id, request.id, call.name, call.arguments)
                .await;
        }
        RequestMethod::Unknown(method) => {
            let response = JsonRpcResponse::error(
                request.id,
                METHOD_NOT_FOUND,
                format!("Method not found: {method}"),
            );
            send_response(&response, stdout).await?;
        }
    }

    Ok(())
}

/// Handles one inbound notification, returning the id of a cancelled request.
///
/// The cancellation key is returned even when `cancel` finds no registration:
/// the loop purges queued responses on that value, and a request that already
/// committed has no registration left to cancel.
pub(super) async fn handle_notification(
    raw_message: Value,
    active_requests: &ActiveRequests,
) -> Option<String> {
    let notification = parse_notification_message(raw_message);
    match notification.method {
        NotificationMethod::Unknown => None,
        NotificationMethod::Cancelled(cancelled) => {
            let request_id = cancelled.request_id.into_lookup_key();
            let _ = active_requests.cancel(&request_id, cancelled.reason).await;
            Some(request_id)
        }
    }
}

/// Removes every queued response and returns those that may still be written.
///
/// stdio forbids sending any further message for a cancelled request
/// (`basic/transports/stdio#cancellation`). A cancellation is read by the same
/// task that writes responses, but a response committed just before it was read
/// can still be sitting in the writer queue, so the queue is emptied here and the
/// cancelled request's frames are dropped.
///
/// This runs for *every* well-formed cancellation, whether or not the request was
/// still registered: once a response has been committed the registration is gone,
/// so `cancel` reports "unknown" even though the frame is still queued. Hanging
/// the purge off that return value would reopen the race.
///
/// The guarantee is bounded by what "cancelled" can mean here. It covers frames
/// that were queued when the notification was *processed*: a frame already
/// written to stdout cannot be recalled, and while the loop is blocked writing
/// one frame it is not reading stdin, so a cancellation may still be waiting on
/// the transport when several responses go out. The spec handles that race by
/// having the client ignore a late response for a request it cancelled.
pub(super) fn take_queued_responses(
    response_rx: &mut mpsc::Receiver<JsonRpcResponse>,
    request_id: &str,
) -> Vec<JsonRpcResponse> {
    let mut queued = Vec::new();
    while let Ok(response) = response_rx.try_recv() {
        queued.push(response);
    }
    retain_not_cancelled(queued, request_id)
}

/// Splits queued responses into the ones that may still be written.
///
/// Ids are compared as typed tracking keys (see [`request_id_key`]), so a
/// cancellation naming `42` matches only a response carrying the number `42`,
/// never the string `"42"`.
fn retain_not_cancelled(responses: Vec<JsonRpcResponse>, request_id: &str) -> Vec<JsonRpcResponse> {
    responses
        .into_iter()
        .filter(|response| request_id_key(&response.id).as_deref() != Some(request_id))
        .collect()
}

/// Capabilities advertised through `server/discover`.
///
/// The tool list is static for the life of the process, so `listChanged` is
/// false and `tools/list` results are publicly cacheable.
fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        tools: serde_json::Map::new(),
    }
}

async fn send_response(response: &JsonRpcResponse, stdout: &mut tokio::io::Stdout) -> Result<()> {
    let response_str = serde_json::to_string(response)?;
    stdout.write_all(response_str.as_bytes()).await?;
    stdout.write_all(b"\n").await?;
    stdout.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(id, serde_json::json!({ "resultType": "complete" }))
    }

    fn ids(responses: &[JsonRpcResponse]) -> Vec<Value> {
        responses
            .iter()
            .map(|response| response.id.clone())
            .collect()
    }

    fn key(id: Value) -> String {
        request_id_key(&id).expect("a JSON-RPC id")
    }

    #[test]
    fn queued_response_for_the_cancelled_request_is_dropped() {
        let kept = retain_not_cancelled(
            vec![
                response(serde_json::json!(1)),
                response(serde_json::json!(2)),
            ],
            &key(serde_json::json!(1)),
        );
        assert_eq!(ids(&kept), vec![serde_json::json!(2)]);
    }

    #[test]
    fn ids_of_different_json_types_are_not_confused() {
        // JSON-RPC ids are typed: cancelling the number `42` must not drop the
        // response for the string `"42"`.
        let kept = retain_not_cancelled(
            vec![
                response(serde_json::json!(42)),
                response(serde_json::json!("42")),
            ],
            &key(serde_json::json!(42)),
        );
        assert_eq!(ids(&kept), vec![serde_json::json!("42")]);

        let kept = retain_not_cancelled(
            vec![
                response(serde_json::json!(42)),
                response(serde_json::json!("42")),
            ],
            &key(serde_json::json!("42")),
        );
        assert_eq!(ids(&kept), vec![serde_json::json!(42)]);
    }

    #[test]
    fn responses_for_other_requests_survive_in_order() {
        let kept = retain_not_cancelled(
            vec![
                response(serde_json::json!("a")),
                response(serde_json::json!(7)),
                response(serde_json::json!("c")),
            ],
            &key(serde_json::json!(7)),
        );
        assert_eq!(
            ids(&kept),
            vec![serde_json::json!("a"), serde_json::json!("c")]
        );
    }

    /// Builds a registry plus the writer queue it feeds.
    fn registry() -> (
        ActiveRequests,
        mpsc::Sender<JsonRpcResponse>,
        mpsc::Receiver<JsonRpcResponse>,
    ) {
        let (tx, rx) = mpsc::channel(8);
        let extra = tx.clone();
        (ActiveRequests::new(tx), extra, rx)
    }

    #[tokio::test]
    async fn cancellation_purges_queued_responses_even_when_nothing_is_registered() {
        // Synthetic post-commit queue state: the frame is queued and the
        // registration is gone, so `cancel` can only report "unknown". The purge
        // must not depend on that boolean.
        let (requests, sender, mut rx) = registry();
        sender
            .send(response(serde_json::json!(7)))
            .await
            .expect("queue the committed response");
        sender
            .send(response(serde_json::json!(8)))
            .await
            .expect("queue another response");

        let cancelled = handle_notification(
            serde_json::json!({
                "method": "notifications/cancelled",
                "params": { "requestId": 7 }
            }),
            &requests,
        )
        .await
        .expect("a cancellation is reported back to the loop");

        assert!(
            !requests.cancel(&cancelled, None).await,
            "nothing is registered, so the request was already committed"
        );
        assert_eq!(
            ids(&take_queued_responses(&mut rx, &cancelled)),
            vec![serde_json::json!(8)],
            "the committed response is dropped, the other one survives"
        );
    }

    #[tokio::test]
    async fn a_cancellation_for_an_unregistered_id_is_still_reported_to_the_loop() {
        // Synthetic reproduction of the post-commit queue state: the frame is
        // queued but no registration exists any more. The loop must purge on any
        // well-formed cancellation, because `cancel` can only report "unknown"
        // here. (The registered-then-committed flow is covered end to end in the
        // `server::runtime` tests, where registrations can be created.)
        let (requests, sender, mut rx) = registry();
        sender
            .send(response(serde_json::json!(1)))
            .await
            .expect("queue a response");

        let cancelled = handle_notification(
            serde_json::json!({
                "method": "notifications/cancelled",
                "params": { "requestId": 1 }
            }),
            &requests,
        )
        .await
        .expect("a cancellation is reported back to the loop");

        assert!(
            !requests.cancel(&cancelled, None).await,
            "nothing is registered"
        );
        assert!(
            take_queued_responses(&mut rx, &cancelled).is_empty(),
            "no frame for the cancelled request is written"
        );
    }
}
