--------------------------- MODULE McpSynthesisPlayback ---------------------------
(***************************************************************************)
(* Models the MCP text_to_speech tool execution with client-side playback. *)
(*                                                                         *)
(* Design principle: the MCP server is responsible for synthesis only.     *)
(* Audio data is returned as a base64-encoded blob in the MCP tool result, *)
(* and the client decodes it and handles playback.                         *)
(*                                                                         *)
(* Data flow:                                                              *)
(*   Server: synthesize -> encode(WAV->base64) -> return in MCP response   *)
(*   Client: receive response -> decode(base64->WAV) -> play -> done       *)
(*                                                                         *)
(* Corresponding implementation:                                           *)
(*   src/mcp/tts_execute.rs   -- handle_text_to_speech                     *)
(*   src/mcp/tool_types.rs    -- ToolCallResult (base64 audio content)     *)
(*                                                                         *)
(* Processes:                                                              *)
(*   User:   non-deterministic ESC (cancel)                                *)
(*   Server: WaitRequest -> Synthesize -> Encode -> Respond                *)
(*   Client: SendRequest -> ReceiveResponse -> Decode -> Play              *)
(***************************************************************************)

EXTENDS Integers, TLC

\* ================================================================
\* Variables
\* ================================================================

VARIABLES server_state, client_state, audio_location,
          synthesis_mode, mode_after_request,
          synthesis_succeeded, user_wants_cancel, pc

vars == << server_state, client_state, audio_location,
           synthesis_mode, mode_after_request,
           synthesis_succeeded, user_wants_cancel, pc >>

Procs == {"user", "server", "client"}

\* ================================================================
\* Invariants
\* ================================================================

TypeOK ==
    /\ server_state \in {"idle", "synthesizing", "encoding", "responding"}
    /\ client_state \in {"requesting", "waiting", "received",
                          "decoding", "playing", "done", "cancelled"}
    /\ audio_location \in {"nowhere", "at_server_raw", "at_server_encoded",
                            "at_client_encoded", "at_client_decoded"}
    /\ synthesis_mode \in {"daemon", "streaming"}
    /\ mode_after_request \in {"unset", "daemon", "streaming"}
    /\ synthesis_succeeded \in BOOLEAN
    /\ user_wants_cancel \in BOOLEAN

AudioDataIntegrity ==
    /\ (audio_location = "at_server_raw" =>
            /\ server_state \in {"synthesizing", "encoding"}
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

PlaybackRequiresAudio ==
    \* Client can only be playing if audio has been fully decoded
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

\* ================================================================
\* Initial State
\* ================================================================

Init ==
    /\ server_state = "idle"
    /\ client_state = "requesting"
    /\ audio_location = "nowhere"
    /\ synthesis_mode = "daemon"
    /\ mode_after_request = "unset"
    /\ synthesis_succeeded = FALSE
    /\ user_wants_cancel = FALSE
    /\ pc = [p \in Procs |->
                CASE p = "user"   -> "UserAction"
                  [] p = "server" -> "WaitRequest"
                  [] p = "client" -> "SendRequest"]

\* ================================================================
\* User Actions
\* ================================================================

UserAction ==
    /\ pc["user"] = "UserAction"
    /\ pc["client"] # "Done_client"
    /\ user_wants_cancel' \in {user_wants_cancel, TRUE}
    /\ UNCHANGED << server_state, client_state, audio_location,
                    synthesis_mode, mode_after_request,
                    synthesis_succeeded, pc >>

\* ================================================================
\* Server Actions
\* ================================================================

WaitRequest ==
    /\ pc["server"] = "WaitRequest"
    /\ client_state = "waiting"
    /\ server_state' = "synthesizing"
    /\ pc' = [pc EXCEPT !["server"] = "SynthesizeAction"]
    /\ UNCHANGED << client_state, audio_location, synthesis_mode, mode_after_request,
                    synthesis_succeeded, user_wants_cancel >>

SynthesizeAction ==
    /\ pc["server"] = "SynthesizeAction"
    /\ \/ (/\ synthesis_succeeded' = TRUE
           /\ audio_location' = "at_server_raw")
       \/ (/\ synthesis_succeeded' = FALSE
           /\ UNCHANGED audio_location)
    /\ pc' = [pc EXCEPT !["server"] = "Encode"]
    /\ UNCHANGED << server_state, client_state, synthesis_mode, mode_after_request,
                    user_wants_cancel >>

Encode ==
    /\ pc["server"] = "Encode"
    /\ IF synthesis_succeeded
       THEN /\ server_state' = "encoding"
            /\ audio_location' = "at_server_encoded"
       ELSE /\ server_state' = "responding"
            /\ UNCHANGED audio_location
    /\ pc' = [pc EXCEPT !["server"] = "Respond"]
    /\ UNCHANGED << client_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, user_wants_cancel >>

Respond ==
    /\ pc["server"] = "Respond"
    /\ IF synthesis_succeeded
       THEN IF user_wants_cancel
            THEN audio_location' = "nowhere"
            ELSE audio_location' = "at_client_encoded"
       ELSE UNCHANGED audio_location
    /\ server_state' = "idle"
    /\ pc' = [pc EXCEPT !["server"] = "Done_server"]
    /\ UNCHANGED << client_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, user_wants_cancel >>

\* ================================================================
\* Client Actions
\* ================================================================

SendRequest ==
    /\ pc["client"] = "SendRequest"
    /\ synthesis_mode' \in {"daemon", "streaming"}
    /\ mode_after_request' = synthesis_mode'
    /\ client_state' = "waiting"
    /\ pc' = [pc EXCEPT !["client"] = "ReceiveResponse"]
    /\ UNCHANGED << server_state, audio_location, synthesis_succeeded, user_wants_cancel >>

ReceiveResponse ==
    /\ pc["client"] = "ReceiveResponse"
    /\ pc["server"] = "Done_server"
    /\ IF user_wants_cancel
       THEN /\ client_state' = "cancelled"
            /\ audio_location' = "nowhere"
            /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
       ELSE IF ~synthesis_succeeded
            THEN /\ client_state' = "done"
                 /\ UNCHANGED audio_location
                 /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
            ELSE /\ client_state' = "received"
                 /\ UNCHANGED audio_location
                 /\ pc' = [pc EXCEPT !["client"] = "Decode"]
    /\ UNCHANGED << server_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, user_wants_cancel >>

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
                    synthesis_succeeded, user_wants_cancel >>

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
                    synthesis_succeeded, user_wants_cancel >>

Playback ==
    /\ pc["client"] = "Playback"
    /\ IF user_wants_cancel
       THEN /\ client_state' = "cancelled"
            /\ audio_location' = "nowhere"
       ELSE /\ client_state' = "done"
            /\ UNCHANGED audio_location
    /\ pc' = [pc EXCEPT !["client"] = "Done_client"]
    /\ UNCHANGED << server_state, synthesis_mode, mode_after_request,
                    synthesis_succeeded, user_wants_cancel >>

\* ================================================================
\* Specification
\* ================================================================

Terminated ==
    /\ pc["server"] = "Done_server"
    /\ pc["client"] = "Done_client"

Next ==
    \/ UserAction
    \/ WaitRequest
    \/ SynthesizeAction
    \/ Encode
    \/ Respond
    \/ SendRequest
    \/ ReceiveResponse
    \/ Decode
    \/ CheckBeforePlay
    \/ Playback
    \/ (Terminated /\ UNCHANGED vars)

Fairness ==
    /\ WF_vars(WaitRequest)
    /\ WF_vars(SynthesizeAction)
    /\ WF_vars(Encode)
    /\ WF_vars(Respond)
    /\ WF_vars(SendRequest)
    /\ WF_vars(ReceiveResponse)
    /\ WF_vars(Decode)
    /\ WF_vars(CheckBeforePlay)
    /\ WF_vars(Playback)

Spec == Init /\ [][Next]_vars /\ Fairness

\* ================================================================
\* Liveness
\* ================================================================

ClientTermination ==
    <>(client_state \in {"done", "cancelled"})

NoOrphanedPlayback ==
    ~(user_wants_cancel
      /\ server_state \in {"synthesizing", "encoding", "responding"}
      /\ client_state = "playing")

CancelStopsPlayback ==
    [](user_wants_cancel /\ client_state = "playing"
       => <>(client_state = "cancelled"))

CancelDoesNotInterruptServer ==
    [](user_wants_cancel /\ server_state \in {"synthesizing", "encoding", "responding"}
       => <>(pc["server"] = "Done_server"))

ServerReturnsToIdle ==
    [](server_state # "idle" => <>(server_state = "idle"))

=============================================================================
