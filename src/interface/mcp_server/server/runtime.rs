use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, Semaphore, mpsc, oneshot};

use crate::interface::mcp_server::protocol::{INTERNAL_ERROR, INVALID_REQUEST, JsonRpcResponse};
use crate::interface::mcp_server::tools::text_to_speech::spawn_non_send_text_to_speech_task;
use crate::interface::mcp_server::tools::types::{ToolCallResult, text_result};

const MAX_CONCURRENT_TOOL_HANDLERS: usize = 4;

/// Message used when a client reuses a request id that is still in flight.
pub const DUPLICATE_REQUEST_ID_MESSAGE: &str = "Duplicate request ID for in-flight request";

fn serialize_result_response(
    id: Value,
    result: ToolCallResult,
    fallback_message: &str,
) -> JsonRpcResponse {
    match serde_json::to_value(result) {
        Ok(value) => JsonRpcResponse::success(id, value),
        Err(_) => JsonRpcResponse::error(id, INTERNAL_ERROR, fallback_message),
    }
}

fn tool_handler_error_result(error: &anyhow::Error) -> ToolCallResult {
    text_result(format!("Tool handler error: {error}"), true)
}

/// Identifies one registration of a request id.
///
/// A request id can be registered again once the previous holder has finished,
/// so "the entry under `request_id`" is not the same thing as "the request this
/// handler started". Every handler carries the ticket it was given and may only
/// finish *its own* registration, which makes a stale handler harmless even if
/// it wakes up after the id has been reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Ticket(u64);

/// Per-request state shared between the cancellation path and the handler that
/// eventually answers the request.
///
/// MCP requires a server to stop sending messages for a cancelled request, so a
/// handler must learn about cancellation *before* it commits a response. The
/// abort channel and the `cancelled` flag live behind one lock, and `commit`
/// decides under that lock whether the request may still be answered.
///
/// The guarantee has a defined shape:
///
/// * `commit` reserves a writer slot *before* it retires the entry, so a commit
///   waiting for queue capacity leaves the request registered and cancellable.
///   Retiring first and then parking on a full queue would strand a cancelled
///   request with no registration left to cancel, yet still able to emit a
///   response later.
/// * The linearization point of "may this request still be answered?" is the
///   ticket-and-cancellation check under this lock, and the enqueue happens
///   under that same lock. `cancel` takes the lock too, so a cancellation can
///   only land before the check (suppressing the response) or after the response
///   is already in the writer queue (where `stdio` drops it). There is no
///   interval in which the entry is gone but the response is not yet queued.
/// * The remaining limit is transport, not registry: a frame already written to
///   stdout cannot be recalled, and while the loop is blocked writing one frame
///   it is not reading stdin, so a cancellation may be processed after several
///   frames were written. The spec covers exactly that race by telling clients to
///   ignore a late response for a request they cancelled.
#[derive(Debug)]
struct InFlightRequest {
    ticket: Ticket,
    abort: Option<oneshot::Sender<String>>,
    cancelled: bool,
}

/// Outcome of registering a request.
///
/// A duplicate is answered immediately and never dispatched; a registered
/// request hands back its ticket and, for cancellable tools, its abort channel.
enum Registration {
    Registered {
        ticket: Ticket,
        abort_rx: Option<oneshot::Receiver<String>>,
    },
    Duplicate,
}

#[derive(Debug, Clone)]
pub struct ActiveRequests {
    in_flight: Arc<Mutex<HashMap<String, InFlightRequest>>>,
    response_sender: mpsc::Sender<JsonRpcResponse>,
    handler_slots: Arc<Semaphore>,
    next_ticket: Arc<AtomicU64>,
}

impl ActiveRequests {
    #[must_use]
    pub fn new(response_sender: mpsc::Sender<JsonRpcResponse>) -> Self {
        Self {
            in_flight: Arc::new(Mutex::new(HashMap::new())),
            response_sender,
            handler_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_TOOL_HANDLERS)),
            next_ticket: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Marks the current holder of `request_id` cancelled and stops its handler.
    ///
    /// Cancellation arrives with a bare request id, so it always refers to
    /// whatever request currently holds that id; that is the request the client
    /// issued. Returns `false` when the id is unknown or already cancelled, which
    /// makes a late cancellation a no-op as the spec's race rules require.
    pub async fn cancel(&self, request_id: &str, reason: Option<String>) -> bool {
        let mut in_flight = self.in_flight.lock().await;
        let Some(entry) = in_flight.get_mut(request_id) else {
            return false;
        };
        if entry.cancelled {
            return false;
        }
        entry.cancelled = true;
        if let Some(sender) = entry.abort.take() {
            let _ = sender.send(reason.unwrap_or_default());
        }
        true
    }

    pub async fn cancel_all_requests(&self, reason: &str) -> usize {
        let mut in_flight = self.in_flight.lock().await;
        let count = in_flight.len();

        for entry in in_flight.values_mut() {
            entry.cancelled = true;
            if let Some(sender) = entry.abort.take() {
                let _ = sender.send(reason.to_string());
            }
        }

        count
    }

    fn allocate_ticket(&self) -> Ticket {
        Ticket(self.next_ticket.fetch_add(1, Ordering::Relaxed) + 1)
    }

    /// Whether `request_id` currently has an answer outstanding.
    ///
    /// The synchronous methods use this to reject a reused id: `tools/call` gets
    /// the same protection from [`Self::register`], which is atomic with the
    /// insertion. A plain check is enough here because only the stdin task
    /// registers requests, so nothing can insert between the check and the answer.
    pub async fn is_in_flight(&self, request_id: &str) -> bool {
        self.in_flight.lock().await.contains_key(request_id)
    }

    /// Registers an in-flight request before its handler starts.
    async fn register(&self, request_id: &str, id: &Value, cancellable: bool) -> Registration {
        // Allocate before taking the map lock: the counter is independent of map
        // contents, so the two locks never nest.
        let ticket = self.allocate_ticket();

        let mut in_flight = self.in_flight.lock().await;
        if in_flight.contains_key(request_id) {
            drop(in_flight);
            // Sending a second request under a live id violates the protocol
            // ("the request ID MUST NOT match the ID of any other request the
            // sender has issued and not yet received a response for"), so this
            // is an invalid request rather than a server-side failure. The
            // in-flight original still answers for itself.
            let _ = self
                .response_sender
                .send(JsonRpcResponse::error(
                    id.clone(),
                    INVALID_REQUEST,
                    DUPLICATE_REQUEST_ID_MESSAGE,
                ))
                .await;
            return Registration::Duplicate;
        }

        let (abort, abort_rx) = if cancellable {
            let (tx, rx) = oneshot::channel::<String>();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        in_flight.insert(
            request_id.to_string(),
            InFlightRequest {
                ticket,
                abort,
                cancelled: false,
            },
        );
        Registration::Registered { ticket, abort_rx }
    }

    /// Publishes `response` unless this registration was cancelled first.
    ///
    /// Only the holder of `ticket` may retire the entry, so a handler that wakes
    /// up after its id was reused cannot answer for, or remove, the newer
    /// request. Returns whether the response was handed to the writer queue.
    ///
    /// The writer slot is reserved *before* the commit point, so the request
    /// stays registered and cancellable while this waits for queue capacity, and
    /// the hand-off under the lock never blocks. Retiring the entry first and
    /// then parking on a full queue would leave a cancelled request with no
    /// registration left to cancel, yet still able to emit a response later.
    async fn commit(&self, request_id: &str, ticket: Ticket, response: JsonRpcResponse) -> bool {
        let Ok(writer_slot) = self.response_sender.reserve().await else {
            // The writer is gone, so there is nowhere to answer. Release the id,
            // but only if it is still this handler's registration: a stale
            // handler must not retire a replacement.
            let mut in_flight = self.in_flight.lock().await;
            if in_flight
                .get(request_id)
                .is_some_and(|entry| entry.ticket == ticket)
            {
                in_flight.remove(request_id);
            }
            return false;
        };

        let mut in_flight = self.in_flight.lock().await;
        if !in_flight
            .get(request_id)
            .is_some_and(|entry| entry.ticket == ticket)
        {
            // Not this handler's registration: it may neither answer nor retire
            // it.
            return false;
        }
        // Retiring is unconditional for the owning ticket: a cancelled request
        // still has to release its id, or the client could never reuse it.
        let cancelled = in_flight
            .remove(request_id)
            .is_some_and(|entry| entry.cancelled);
        if cancelled {
            // Dropping the permit returns the capacity to the queue.
            return false;
        }

        // Enqueue while the registration lock is still held. `cancel` takes the
        // same lock, so a cancellation either lands before this point (and the
        // entry is marked cancelled, suppressing the response) or after the
        // response is already in the writer queue, where `stdio` drops it. Sending
        // outside the lock would leave an interval in which the entry is gone but
        // the response is not yet queued, and a cancellation in that interval
        // could not prevent the write. A reserved permit never blocks, so holding
        // the lock across the send cannot stall cancellation on backpressure.
        writer_slot.send(response);
        true
    }

    pub async fn spawn_tool_handler(
        &self,
        request_id: String,
        id: Value,
        tool_name: String,
        arguments: Value,
    ) {
        let permit = match Arc::clone(&self.handler_slots).try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                let response =
                    JsonRpcResponse::error(id, INTERNAL_ERROR, "Too many concurrent tool handlers");
                let _ = self.response_sender.send(response).await;
                return;
            }
        };

        // Only `text_to_speech` runs long enough to be aborted; the other tools
        // answer from a single daemon round trip.
        let cancellable = tool_name == "text_to_speech";
        let (ticket, abort_rx) = match self.register(&request_id, &id, cancellable).await {
            Registration::Registered { ticket, abort_rx } => (ticket, abort_rx),
            Registration::Duplicate => return,
        };

        let active_requests = self.clone();
        if cancellable {
            spawn_non_send_text_to_speech_task(move || {
                Box::pin(async move {
                    let _permit = permit;
                    let result =
                        crate::interface::mcp_server::tools::registry::execute_tool_request(
                            &tool_name, arguments, abort_rx,
                        )
                        .await;
                    let response = match result {
                        Ok(tool_result) => serialize_result_response(
                            id,
                            tool_result,
                            "Failed to serialize response",
                        ),
                        Err(error) => serialize_result_response(
                            id,
                            tool_handler_error_result(&error),
                            "Failed to serialize error response",
                        ),
                    };

                    active_requests.commit(&request_id, ticket, response).await;
                })
            });
            return;
        }

        tokio::spawn(async move {
            let _permit = permit;
            let result = crate::interface::mcp_server::tools::registry::execute_send_tool_request(
                &tool_name, arguments,
            )
            .await;

            let response = match result {
                Ok(tool_result) => {
                    serialize_result_response(id, tool_result, "Failed to serialize response")
                }
                Err(error) => serialize_result_response(
                    id,
                    tool_handler_error_result(&error),
                    "Failed to serialize error response",
                ),
            };

            active_requests.commit(&request_id, ticket, response).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;
    use serde_json::json;

    fn active_requests() -> (ActiveRequests, mpsc::Receiver<JsonRpcResponse>) {
        let (tx, rx) = mpsc::channel(8);
        (ActiveRequests::new(tx), rx)
    }

    async fn register(
        requests: &ActiveRequests,
        request_id: &str,
        id: u32,
        cancellable: bool,
    ) -> (Ticket, Option<oneshot::Receiver<String>>) {
        match requests.register(request_id, &json!(id), cancellable).await {
            Registration::Registered { ticket, abort_rx } => (ticket, abort_rx),
            Registration::Duplicate => panic!("request {request_id} was rejected as a duplicate"),
        }
    }

    fn answer(id: u32) -> JsonRpcResponse {
        JsonRpcResponse::success(json!(id), json!({}))
    }

    fn ids_of(responses: &[JsonRpcResponse]) -> Vec<Value> {
        responses
            .iter()
            .map(|response| response.id.clone())
            .collect()
    }

    #[tokio::test]
    async fn cancel_before_commit_suppresses_the_response() {
        let (requests, mut responses) = active_requests();
        let (ticket, abort_rx) = register(&requests, "1", 1, true).await;
        let mut abort_rx = abort_rx.expect("cancellable request has an abort channel");

        assert!(requests.cancel("1", Some("user abort".to_string())).await);

        assert!(!requests.commit("1", ticket, answer(1)).await);
        assert!(responses.try_recv().is_err(), "no response may be emitted");
        assert_eq!(abort_rx.try_recv().expect("abort delivered"), "user abort");
    }

    #[tokio::test]
    async fn commit_wins_the_race_and_late_cancel_is_a_no_op() {
        let (requests, mut responses) = active_requests();
        let (ticket, _abort_rx) = register(&requests, "2", 2, true).await;

        assert!(requests.commit("2", ticket, answer(2)).await);
        assert!(responses.try_recv().is_ok(), "response is published");

        assert!(
            !requests.cancel("2", None).await,
            "an answered request cannot be cancelled"
        );
    }

    #[tokio::test]
    async fn duplicate_request_id_is_rejected_before_dispatch() {
        let (requests, mut responses) = active_requests();
        let _first = register(&requests, "3", 3, true).await;

        assert!(matches!(
            requests.register("3", &json!(3), true).await,
            Registration::Duplicate
        ));

        let response = responses.try_recv().expect("duplicate is answered");
        // A reused live id is a client-side protocol violation; the server was
        // not unable to do its job.
        assert_eq!(response.error.expect("error present").code, INVALID_REQUEST);
    }

    #[tokio::test]
    async fn non_cancellable_tool_gets_no_abort_channel() {
        let (requests, _responses) = active_requests();
        let (_ticket, abort_rx) = register(&requests, "4", 4, false).await;
        assert!(abort_rx.is_none());

        assert!(requests.cancel("4", None).await);
    }

    #[tokio::test]
    async fn cancel_all_marks_every_in_flight_request() {
        let (requests, mut responses) = active_requests();
        let (first_ticket, first_abort) = register(&requests, "5", 5, true).await;
        let mut first_abort = first_abort.expect("cancellable request has an abort channel");
        let (second_ticket, _second_abort) = register(&requests, "6", 6, false).await;

        assert_eq!(requests.cancel_all_requests("shutdown").await, 2);
        assert_eq!(first_abort.try_recv().expect("abort delivered"), "shutdown");

        assert!(!requests.commit("5", first_ticket, answer(5)).await);
        assert!(!requests.commit("6", second_ticket, answer(6)).await);
        assert!(responses.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_commit_parked_on_a_full_queue_stays_cancellable_and_never_answers() {
        // `commit` reserves a writer slot *before* it retires the registration,
        // so waiting for a full queue never leaves a cancelled request without a
        // registration to cancel. This pins both halves: the cancellation is
        // accepted while the commit waits, and the response is still suppressed.
        let (tx, mut responses) = mpsc::channel(1);
        let requests = ActiveRequests::new(tx);
        let (ticket, _abort_rx) = register(&requests, "parked", 1, true).await;

        requests
            .response_sender
            .send(answer(0))
            .await
            .expect("seed the writer queue");

        let mut committing = Box::pin(requests.commit("parked", ticket, answer(1)));
        assert!(
            committing.as_mut().now_or_never().is_none(),
            "commit must wait for writer capacity"
        );

        assert!(
            requests
                .cancel("parked", Some("user abort".to_string()))
                .await,
            "the registration is still live while commit waits for capacity"
        );

        // Draining frees the slot; the parked commit wakes, sees the
        // cancellation, and must stay silent.
        assert!(
            responses.recv().await.is_some(),
            "the seeded frame is written"
        );
        let published = loop {
            if let Some(published) = committing.as_mut().now_or_never() {
                break published;
            }
            tokio::task::yield_now().await;
        };
        assert!(!published, "a cancelled request is never answered");
        assert!(responses.try_recv().is_err(), "no second frame is written");
    }

    #[tokio::test]
    async fn a_stale_handler_cannot_release_a_replacement_when_the_writer_is_gone() {
        // The reserve-failure path also retires the registration, so it needs the
        // same ticket guard: losing the writer must not let a stale handler free
        // an id that now belongs to someone else.
        let (tx, responses) = mpsc::channel(1);
        drop(responses);
        let requests = ActiveRequests::new(tx);
        let (_ticket, _abort_rx) = register(&requests, "orphan", 1, false).await;

        assert!(
            !requests.commit("orphan", Ticket(u64::MAX), answer(1)).await,
            "a foreign ticket cannot answer"
        );
        assert!(
            requests.cancel("orphan", None).await,
            "the registration survives the failed commit"
        );
    }

    #[tokio::test]
    async fn a_committed_response_is_purged_when_the_cancellation_arrives() {
        // The whole path with real registrations: register, commit (which queues
        // the response and retires the entry), then the client's cancellation
        // arrives. `cancel` can only report "unknown" at that point, yet the
        // queued frame must not reach stdout -- which is why the purge is keyed on
        // the notification, not on that boolean.
        use crate::interface::mcp_server::protocol::request_id_key;
        use crate::interface::mcp_server::server::stdio::{
            handle_notification, take_queued_responses,
        };

        let (requests, mut responses) = active_requests();
        let first = request_id_key(&json!(7)).expect("numeric id has a key");
        let second = request_id_key(&json!(8)).expect("numeric id has a key");

        let (first_ticket, _first_abort) = register(&requests, &first, 7, true).await;
        let (second_ticket, _second_abort) = register(&requests, &second, 8, true).await;
        assert!(requests.commit(&first, first_ticket, answer(7)).await);
        assert!(requests.commit(&second, second_ticket, answer(8)).await);

        let cancelled = handle_notification(
            json!({
                "method": "notifications/cancelled",
                "params": { "requestId": 7 }
            }),
            &requests,
        )
        .await
        .expect("every well-formed cancellation is reported to the loop");

        assert_eq!(cancelled, first, "the notification names the request id");
        assert!(
            !requests.cancel(&cancelled, None).await,
            "the committed request has no registration left"
        );

        let kept = take_queued_responses(&mut responses, &cancelled);
        assert_eq!(
            ids_of(&kept),
            vec![json!(8)],
            "only the other request answers"
        );
    }

    #[tokio::test]
    async fn an_in_flight_id_is_visible_until_its_handler_finishes() {
        let (requests, _responses) = active_requests();
        assert!(!requests.is_in_flight("late").await, "unknown ids are free");

        let (ticket, _abort_rx) = register(&requests, "late", 1, false).await;
        assert!(
            requests.is_in_flight("late").await,
            "a registered id is in flight"
        );

        assert!(requests.commit("late", ticket, answer(1)).await);
        assert!(
            !requests.is_in_flight("late").await,
            "a committed id is free again"
        );
    }

    #[tokio::test]
    async fn unknown_cancellation_is_ignored() {
        let (requests, _responses) = active_requests();
        assert!(!requests.cancel("missing", None).await);
    }

    #[tokio::test]
    async fn a_cancelled_request_keeps_its_id_until_its_handler_finishes() {
        // MCP forbids reusing an id while its response is outstanding. Cancelling
        // does not release the id: the handler still has to wind down and retire
        // its registration, and only then may the id be used again.
        let (requests, _responses) = active_requests();
        let (ticket, _abort_rx) = register(&requests, "busy", 1, true).await;
        assert!(requests.cancel("busy", None).await);

        assert!(
            matches!(
                requests.register("busy", &json!(2), true).await,
                Registration::Duplicate
            ),
            "the id stays occupied while the cancelled handler is still winding down"
        );

        assert!(!requests.commit("busy", ticket, answer(1)).await);
        let (new_ticket, new_abort) = register(&requests, "busy", 2, true).await;
        let mut new_abort = new_abort.expect("fresh abort channel");
        assert!(
            new_abort.try_recv().is_err(),
            "the old cancellation must not reach the new request"
        );
        assert!(requests.commit("busy", new_ticket, answer(2)).await);
    }

    #[tokio::test]
    async fn a_stale_handler_cannot_answer_for_a_reused_request_id() {
        // This is a defensive unit test over the ticket guard, not a captured
        // handler interleaving: it calls `commit` twice with the first request's
        // ticket, which a well-behaved handler would never do. It pins the
        // implementation invariant that makes a late wake-up harmless -- a
        // registration is only retired by a matching ticket, and a cancelled
        // registration keeps occupying its id until its handler retires it -- so
        // the guard is what protects B if A's handler ever does come back.
        let (requests, mut responses) = active_requests();
        let (stale_ticket, _stale_abort) = register(&requests, "reused", 1, true).await;
        assert!(requests.cancel("reused", None).await);
        assert!(!requests.commit("reused", stale_ticket, answer(1)).await);

        let (live_ticket, _live_abort) = register(&requests, "reused", 2, true).await;

        assert!(
            !requests.commit("reused", stale_ticket, answer(1)).await,
            "a stale ticket must not retire the replacement registration"
        );
        assert!(
            requests.cancel("reused", None).await,
            "the replacement is still registered and cancellable"
        );

        // The replacement's own ticket is the only one that can retire it, and
        // by now it is cancelled, so even the live ticket stays silent.
        assert!(!requests.commit("reused", live_ticket, answer(2)).await);
        assert!(
            responses.try_recv().is_err(),
            "neither the stale nor the cancelled registration may be answered"
        );
    }

    #[tokio::test]
    async fn a_replaced_registration_is_not_retired_by_the_old_ticket() {
        // Two registrations of one id: only the current holder's ticket retires
        // it. The retired ticket is rejected even though the entry exists.
        let (requests, mut responses) = active_requests();
        let (old_ticket, _old_abort) = register(&requests, "slot", 1, false).await;

        // A ticket that was never issued cannot retire the entry either.
        assert!(!requests.commit("slot", Ticket(u64::MAX), answer(0)).await);
        assert!(
            responses.try_recv().is_err(),
            "a foreign ticket must not answer the request"
        );

        assert!(requests.commit("slot", old_ticket, answer(1)).await);
        let first = responses.try_recv().expect("first response is published");
        assert_eq!(first.id, json!(1), "the first registration answers id 1");

        let (new_ticket, _new_abort) = register(&requests, "slot", 2, false).await;
        assert!(
            !requests.commit("slot", old_ticket, answer(1)).await,
            "the retired ticket may not act on the new registration"
        );
        assert!(
            responses.try_recv().is_err(),
            "the stale ticket must not emit a second response"
        );

        assert!(
            requests.commit("slot", new_ticket, answer(2)).await,
            "only the current ticket retires the id"
        );
        let second = responses
            .try_recv()
            .expect("replacement response is published");
        assert_eq!(
            second.id,
            json!(2),
            "the replacement answers its own id, exactly once"
        );
        assert!(
            responses.try_recv().is_err(),
            "each registration answers at most once"
        );
    }
}
