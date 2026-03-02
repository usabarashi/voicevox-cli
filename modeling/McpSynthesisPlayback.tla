--------------------------- MODULE McpSynthesisPlayback ---------------------------
(***************************************************************************)
(* Models MCP text_to_speech with bounded multi-request sessions,          *)
(* client retries, and server timeout/crash recovery paths.                *)
(***************************************************************************)

EXTENDS Integers, TLC

CONSTANTS
    MAX_REQUESTS,
    MAX_CLIENT_RETRIES

ASSUME MAX_REQUESTS \in Nat /\ MAX_REQUESTS > 0
ASSUME MAX_CLIENT_RETRIES \in Nat

VARIABLES server_state, client_state, audio_location,
          synthesis_mode, mode_after_request,
          synthesis_succeeded, server_result,
          user_wants_cancel, request_index, retry_count, pc

vars == << server_state, client_state, audio_location,
           synthesis_mode, mode_after_request,
           synthesis_succeeded, server_result,
           user_wants_cancel, request_index, retry_count, pc >>

Procs == {"user", "server", "client"}

TypeOK ==
    /\ server_state \in {"idle", "synthesizing", "encoding", "responding", "crashed"}
    /\ client_state \in {"requesting", "waiting", "received",
                          "decoding", "playing", "done", "cancelled"}
    /\ audio_location \in {"nowhere", "at_server_raw", "at_server_encoded",
                            "at_client_encoded", "at_client_decoded"}
    /\ synthesis_mode \in {"daemon", "streaming"}
    /\ mode_after_request \in {"unset", "daemon", "streaming"}
    /\ synthesis_succeeded \in BOOLEAN
    /\ server_result \in {"none", "ok", "synthesis_failed", "timeout", "crash"}
    /\ user_wants_cancel \in BOOLEAN
    /\ request_index \in 0..MAX_REQUESTS
    /\ retry_count \in 0..MAX_CLIENT_RETRIES

AudioDataIntegrity ==
    /\ (audio_location = "at_server_raw" =>
            /\ server_state \in {"synthesizing", "encoding", "responding"}
            /\ client_state \in {"requesting", "waiting"})
    /\ (audio_location = "at_server_encoded" =>
            /\ server_state \in {"encoding", "responding"}
            /\ client_state \in {"requesting", "waiting"})
    /\ (audio_location = "at_client_encoded" =>
            /\ server_state = "idle"
            /\ client_state \in {"waiting", "received", "decoding", "cancelled"})
    /\ (audio_location = "at_client_decoded" =>
            /\ server_state = "idle"
            /\ client_state \in {"decoding", "playing", "done", "cancelled"})

NoOrphanedPlayback ==
    ~(user_wants_cancel
      /\ server_state \in {"synthesizing", "encoding", "responding", "crashed"}
      /\ client_state = "playing")

PlaybackRequiresAudio ==
    client_state = "playing" => audio_location = "at_client_decoded"

CompletionRequiresAudio ==
    (client_state = "done" /\ synthesis_succeeded) => audio_location = "at_client_decoded"

EncodingDecodingSeparation ==
    /\ (server_state = "encoding" => audio_location \in {"at_server_raw", "at_server_encoded"})
    /\ (client_state = "decoding" => audio_location \in {"at_client_encoded", "at_client_decoded"})

ModeStabilityAfterRequest ==
    /\ (mode_after_request = "unset"
        \/ synthesis_mode = mode_after_request)
    /\ (client_state = "requesting" \/ mode_after_request # "unset")

CancelledCleansAudio ==
    client_state = "cancelled" => audio_location = "nowhere"

RequestIndexBounded ==
    request_index <= MAX_REQUESTS

Init ==
    /\ server_state = "idle"
    /\ client_state = "requesting"
    /\ audio_location = "nowhere"
    /\ synthesis_mode = "daemon"
    /\ mode_after_request = "unset"
    /\ synthesis_succeeded = FALSE
    /\ server_result = "none"
    /\ user_wants_cancel = FALSE
    /\ request_index = 0
    /\ retry_count = 0
    /\ pc = [p \in Procs |->
                CASE p = "user"   -> "UserAction"
                  [] p = "server" -> "WaitRequest"
                  [] p = "client" -> "SendRequest"]

UserAction ==
    /\ pc["user"] = "UserAction"
    /\ request_index < MAX_REQUESTS
    /\ pc["client"] # "Done_client"
    /\ user_wants_cancel' \in {user_wants_cancel, TRUE}
    /\ UNCHANGED << server_state, client_state, audio_location,
                    synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    request_index, retry_count, pc >>

WaitRequest ==
    /\ pc["server"] = "WaitRequest"
    /\ client_state = "waiting"
    /\ server_state' = "synthesizing"
    /\ synthesis_succeeded' = FALSE
    /\ server_result' = "none"
    /\ pc' = [pc EXCEPT !["server"] = "SynthesizeAction"]
    /\ UNCHANGED << client_state, audio_location, synthesis_mode, mode_after_request,
                    user_wants_cancel, request_index, retry_count >>

SynthesizeAction ==
    /\ pc["server"] = "SynthesizeAction"
    /\ \/ /\ synthesis_succeeded' = TRUE
          /\ server_result' = "ok"
          /\ audio_location' = "at_server_raw"
          /\ server_state' = "synthesizing"
          /\ pc' = [pc EXCEPT !["server"] = "Encode"]
       \/ /\ synthesis_succeeded' = FALSE
          /\ server_result' = "synthesis_failed"
          /\ UNCHANGED audio_location
          /\ server_state' = "synthesizing"
          /\ pc' = [pc EXCEPT !["server"] = "Encode"]
       \/ /\ synthesis_succeeded' = FALSE
          /\ server_result' = "timeout"
          /\ UNCHANGED audio_location
          /\ server_state' = "responding"
          /\ pc' = [pc EXCEPT !["server"] = "Respond"]
       \/ /\ synthesis_succeeded' = FALSE
          /\ server_result' = "crash"
          /\ audio_location' = "nowhere"
          /\ server_state' = "crashed"
          /\ pc' = [pc EXCEPT !["server"] = "Recover"]
    /\ UNCHANGED << client_state, synthesis_mode, mode_after_request,
                    user_wants_cancel, request_index, retry_count >>

Encode ==
    /\ pc["server"] = "Encode"
    /\ IF server_result = "ok"
       THEN /\ server_state' = "encoding"
            /\ audio_location' = "at_server_encoded"
       ELSE /\ server_state' = "responding"
            /\ UNCHANGED audio_location
    /\ pc' = [pc EXCEPT !["server"] = "Respond"]
    /\ UNCHANGED << client_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    user_wants_cancel, request_index, retry_count >>

Respond ==
    /\ pc["server"] = "Respond"
    /\ IF server_result = "ok"
       THEN IF user_wants_cancel
            THEN audio_location' = "nowhere"
            ELSE audio_location' = "at_client_encoded"
       ELSE UNCHANGED audio_location
    /\ server_state' = "idle"
    /\ pc' = [pc EXCEPT !["server"] = "Done_server"]
    /\ UNCHANGED << client_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    user_wants_cancel, request_index, retry_count >>

Recover ==
    /\ pc["server"] = "Recover"
    /\ server_state = "crashed"
    /\ server_state' = "idle"
    /\ audio_location' = "nowhere"
    /\ pc' = [pc EXCEPT !["server"] = "Done_server"]
    /\ UNCHANGED << client_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    user_wants_cancel, request_index, retry_count >>

SendRequest ==
    /\ pc["client"] = "SendRequest"
    /\ request_index < MAX_REQUESTS
    /\ IF mode_after_request = "unset"
       THEN /\ synthesis_mode' \in {"daemon", "streaming"}
            /\ mode_after_request' = synthesis_mode'
       ELSE /\ UNCHANGED << synthesis_mode, mode_after_request >>
    /\ client_state' = "waiting"
    /\ pc' = [pc EXCEPT !["client"] = "ReceiveResponse",
                        !["server"] = "WaitRequest"]
    /\ UNCHANGED << server_state, audio_location, synthesis_succeeded, server_result,
                    user_wants_cancel, request_index, retry_count >>

ReceiveResponse ==
    /\ pc["client"] = "ReceiveResponse"
    /\ pc["server"] = "Done_server"
    /\ IF user_wants_cancel
       THEN /\ client_state' = "cancelled"
            /\ audio_location' = "nowhere"
            /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
            /\ UNCHANGED retry_count
       ELSE IF server_result = "ok"
            THEN /\ client_state' = "received"
                 /\ UNCHANGED audio_location
                 /\ pc' = [pc EXCEPT !["client"] = "Decode"]
                 /\ UNCHANGED retry_count
            ELSE IF retry_count < MAX_CLIENT_RETRIES
                 THEN /\ client_state' = "requesting"
                      /\ retry_count' = retry_count + 1
                      /\ pc' = [pc EXCEPT !["client"] = "SendRequest"]
                      /\ UNCHANGED audio_location
                 ELSE /\ client_state' = "done"
                      /\ UNCHANGED audio_location
                      /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
                      /\ UNCHANGED retry_count
    /\ UNCHANGED << server_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    user_wants_cancel, request_index >>

Decode ==
    /\ pc["client"] = "Decode"
    /\ IF user_wants_cancel
       THEN /\ client_state' = "cancelled"
            /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
            /\ audio_location' = "nowhere"
       ELSE /\ client_state' = "decoding"
            /\ audio_location' = "at_client_decoded"
            /\ pc' = [pc EXCEPT !["client"] = "CheckBeforePlay"]
    /\ UNCHANGED << server_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    user_wants_cancel, request_index, retry_count >>

CheckBeforePlay ==
    /\ pc["client"] = "CheckBeforePlay"
    /\ IF user_wants_cancel
       THEN /\ client_state' = "cancelled"
            /\ audio_location' = "nowhere"
            /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
       ELSE /\ client_state' = "playing"
            /\ UNCHANGED audio_location
            /\ pc' = [pc EXCEPT !["client"] = "Playback"]
    /\ UNCHANGED << server_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    user_wants_cancel, request_index, retry_count >>

Playback ==
    /\ pc["client"] = "Playback"
    /\ IF user_wants_cancel
       THEN /\ client_state' = "cancelled"
            /\ audio_location' = "nowhere"
       ELSE /\ client_state' = "done"
            /\ UNCHANGED audio_location
    /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
    /\ UNCHANGED << server_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, server_result,
                    user_wants_cancel, request_index, retry_count >>

FinalizeRequest ==
    /\ pc["client"] = "Done_client"
    /\ pc["server"] = "Done_server"
    /\ request_index < MAX_REQUESTS
    /\ request_index' = request_index + 1
    /\ IF request_index + 1 < MAX_REQUESTS
       THEN /\ client_state' = "requesting"
            /\ audio_location' = "nowhere"
            /\ mode_after_request' = "unset"
            /\ user_wants_cancel' = FALSE
            /\ retry_count' = 0
            /\ synthesis_succeeded' = FALSE
            /\ server_result' = "none"
            /\ pc' = [pc EXCEPT !["client"] = "SendRequest",
                                !["server"] = "WaitRequest"]
       ELSE /\ UNCHANGED << client_state, audio_location, mode_after_request,
                            user_wants_cancel, retry_count,
                            synthesis_succeeded, server_result, pc >>
    /\ UNCHANGED << server_state, synthesis_mode >>

Terminated ==
    /\ request_index = MAX_REQUESTS
    /\ pc["server"] = "Done_server"
    /\ pc["client"] = "Done_client"

Next ==
    \/ UserAction
    \/ WaitRequest
    \/ SynthesizeAction
    \/ Encode
    \/ Respond
    \/ Recover
    \/ SendRequest
    \/ ReceiveResponse
    \/ Decode
    \/ CheckBeforePlay
    \/ Playback
    \/ FinalizeRequest
    \/ (Terminated /\ UNCHANGED vars)

Fairness ==
    /\ WF_vars(WaitRequest)
    /\ WF_vars(SynthesizeAction)
    /\ WF_vars(Encode)
    /\ WF_vars(Respond)
    /\ WF_vars(Recover)
    /\ WF_vars(SendRequest)
    /\ WF_vars(ReceiveResponse)
    /\ WF_vars(Decode)
    /\ WF_vars(CheckBeforePlay)
    /\ WF_vars(Playback)
    /\ WF_vars(FinalizeRequest)

Spec == Init /\ [][Next]_vars /\ Fairness

ClientTermination ==
    <>(client_state \in {"done", "cancelled"})

CancelStopsPlayback ==
    [](user_wants_cancel /\ client_state = "playing"
       => <>(client_state = "cancelled"))

CancelEventuallyTerminatesClient ==
    [](user_wants_cancel /\ pc["client"] # "Done_client"
       => <>(client_state = "cancelled"))

CancelDoesNotInterruptServer ==
    [](user_wants_cancel /\ server_state \in {"synthesizing", "encoding", "responding", "crashed"}
       => <>(pc["server"] = "Done_server"))

ServerReturnsToIdle ==
    [](server_state # "idle" => <>(server_state = "idle"))

SessionEventuallyTerminates ==
    <>(request_index = MAX_REQUESTS /\ pc["client"] = "Done_client")

=============================================================================
