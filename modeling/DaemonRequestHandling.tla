--------------------------- MODULE DaemonRequestHandling ---------------------------
(***************************************************************************)
(* Models the VOICEVOX daemon's concurrent request handling.               *)
(*                                                                         *)
(* Corresponding implementation:                                           *)
(*   src/daemon/server.rs        -- accept_loop, semaphore, handle_client  *)
(*   src/daemon/state/policy.rs  -- SerializedSynthesisPolicy (Mutex)      *)
(*   src/daemon/state/executor.rs -- load/synthesize/unload sequence       *)
(*   src/ipc.rs                  -- DaemonRequest / DaemonResponse         *)
(*                                                                         *)
(* Key properties verified:                                                *)
(*   - MutexExclusion:  at most 1 synthesis executing at any time          *)
(*   - SemaphoreBound:  at most MAX_CLIENTS concurrent handlers            *)
(*   - ModelCleanup:    model always unloaded after synthesis attempt       *)
(*   - ClientTermination: every client eventually reaches a terminal state *)
(*                                                                         *)
(* Process: ClientHandler(c) for c in Clients                              *)
(*   AcquirePermit -> ChooseRequest ->                                     *)
(*     [Synthesize path] AcquireMutex -> LoadModel -> Synthesize ->        *)
(*                       UnloadModel -> ReleaseMutex -> SendResponse       *)
(*     [List path] SendResponse                                            *)
(***************************************************************************)

EXTENDS Integers, FiniteSets, TLC

CONSTANTS
    MAX_CLIENTS,    \* Semaphore capacity (32 in production, 3 for checking)
    Clients         \* Set of client IDs (one more than MAX_CLIENTS to test blocking)

ASSUME MAX_CLIENTS \in Nat /\ MAX_CLIENTS > 0
ASSUME Clients # {} /\ IsFiniteSet(Clients)

RequestTypes == {"Synthesize", "ListSpeakers", "ListModels"}
SynthesisOutcomes == {"success", "load_failed", "synthesis_failed", "invalid_target"}

\* ================================================================
\* Variables
\* ================================================================

VARIABLES semaphore, mutex_holder, model_loaded,
          client_state, request_type, synthesis_outcome, response, pc

vars == << semaphore, mutex_holder, model_loaded,
           client_state, request_type, synthesis_outcome, response, pc >>

\* ================================================================
\* Invariants and Properties (define block)
\* ================================================================

TypeOK ==
    /\ semaphore \in 0..MAX_CLIENTS
    /\ mutex_holder \in Clients \cup {"nobody"}
    /\ model_loaded \in BOOLEAN
    /\ \A c \in Clients:
        /\ client_state[c] \in {"idle", "has_permit", "waiting_mutex",
               "loading_model", "synthesizing", "unloading_model",
               "responding", "done", "aborted"}
        /\ request_type[c] \in RequestTypes
        /\ synthesis_outcome[c] \in SynthesisOutcomes
        /\ response[c] \in {"none", "ok", "error"}

ActiveHandlers ==
    {c \in Clients: client_state[c] \notin {"idle", "done", "aborted"}}

MutexExclusion ==
    Cardinality({c \in Clients:
        pc[c] \in {"LoadModel", "Synthesize", "UnloadModel", "ReleaseMutex"}
    }) <= 1

SemaphoreBound ==
    Cardinality(ActiveHandlers) <= MAX_CLIENTS

SemaphoreConsistency ==
    semaphore = MAX_CLIENTS - Cardinality(ActiveHandlers)

ModelCleanup ==
    \* At the point of releasing the mutex, the model has been unloaded
    \A c \in Clients:
        (pc[c] = "ReleaseMutex") => ~model_loaded

MutexConsistency ==
    \* Mutex held by c iff c's pc is in a mutex-protected step
    \A c \in Clients:
        (mutex_holder = c) <=>
        (pc[c] \in {"LoadModel", "Synthesize", "UnloadModel", "ReleaseMutex"})

ModelOnlyUnderMutex ==
    model_loaded => (mutex_holder # "nobody")

ClientStatePcConsistency ==
    \A c \in Clients:
        /\ (pc[c] = "LoadModel" => client_state[c] \in {"waiting_mutex", "loading_model"})
        /\ (pc[c] = "Synthesize" => client_state[c] \in {"loading_model", "synthesizing"})
        /\ (pc[c] = "UnloadModel" => client_state[c] \in {"synthesizing", "unloading_model"})
        /\ (pc[c] = "ReleaseMutex" => client_state[c] \in {"responding", "unloading_model"})

DoneHasResponse ==
    \A c \in Clients:
        client_state[c] = "done" => response[c] \in {"ok", "error"}

DonePcConsistency ==
    \A c \in Clients:
        pc[c] = "Done" => client_state[c] \in {"done", "aborted"}

\* ================================================================
\* Initial State
\* ================================================================

Init ==
    /\ semaphore = MAX_CLIENTS
    /\ mutex_holder = "nobody"
    /\ model_loaded = FALSE
    /\ client_state = [c \in Clients |-> "idle"]
    /\ request_type = [c \in Clients |-> "Synthesize"]
    /\ synthesis_outcome = [c \in Clients |-> "success"]
    /\ response = [c \in Clients |-> "none"]
    /\ pc = [c \in Clients |-> "AcquirePermit"]

\* ================================================================
\* Actions
\* ================================================================

AcquirePermit(c) ==
    /\ pc[c] = "AcquirePermit"
    /\ semaphore > 0
    /\ semaphore' = semaphore - 1
    /\ client_state' = [client_state EXCEPT ![c] = "has_permit"]
    /\ pc' = [pc EXCEPT ![c] = "ChooseRequest"]
    /\ UNCHANGED << mutex_holder, model_loaded, request_type, synthesis_outcome, response >>

ChooseRequest(c) ==
    /\ pc[c] = "ChooseRequest"
    /\ \E rt \in RequestTypes:
        /\ request_type' = [request_type EXCEPT ![c] = rt]
        /\ IF rt = "Synthesize"
           THEN /\ client_state' = [client_state EXCEPT ![c] = "waiting_mutex"]
                /\ pc' = [pc EXCEPT ![c] = "AcquireMutex"]
                /\ UNCHANGED response
           ELSE /\ client_state' = [client_state EXCEPT ![c] = "responding"]
                /\ response' = [response EXCEPT ![c] = "ok"]
                /\ pc' = [pc EXCEPT ![c] = "SendResponse"]
    /\ UNCHANGED << semaphore, mutex_holder, model_loaded, synthesis_outcome >>

AcquireMutex(c) ==
    /\ pc[c] = "AcquireMutex"
    /\ mutex_holder = "nobody"
    /\ mutex_holder' = c
    /\ pc' = [pc EXCEPT ![c] = "LoadModel"]
    /\ UNCHANGED << semaphore, model_loaded, client_state, request_type,
                    synthesis_outcome, response >>

LoadModel(c) ==
    /\ pc[c] = "LoadModel"
    /\ \E outcome \in {"success", "load_failed", "invalid_target"}:
        /\ synthesis_outcome' = [synthesis_outcome EXCEPT ![c] = outcome]
        /\ IF outcome = "invalid_target"
           THEN /\ client_state' = [client_state EXCEPT ![c] = "responding"]
                /\ response' = [response EXCEPT ![c] = "error"]
                /\ pc' = [pc EXCEPT ![c] = "ReleaseMutex"]
                /\ UNCHANGED model_loaded
           ELSE IF outcome = "load_failed"
                THEN /\ client_state' = [client_state EXCEPT ![c] = "responding"]
                     /\ response' = [response EXCEPT ![c] = "error"]
                     /\ pc' = [pc EXCEPT ![c] = "ReleaseMutex"]
                     /\ UNCHANGED model_loaded
                ELSE /\ client_state' = [client_state EXCEPT ![c] = "loading_model"]
                     /\ model_loaded' = TRUE
                     /\ pc' = [pc EXCEPT ![c] = "Synthesize"]
                     /\ UNCHANGED response
    /\ UNCHANGED << semaphore, mutex_holder, request_type >>

Synthesize(c) ==
    /\ pc[c] = "Synthesize"
    /\ client_state' = [client_state EXCEPT ![c] = "synthesizing"]
    /\ \E outcome \in {"success", "synthesis_failed"}:
        synthesis_outcome' = [synthesis_outcome EXCEPT ![c] = outcome]
    /\ pc' = [pc EXCEPT ![c] = "UnloadModel"]
    /\ UNCHANGED << semaphore, mutex_holder, model_loaded, request_type, response >>

UnloadModel(c) ==
    /\ pc[c] = "UnloadModel"
    /\ client_state' = [client_state EXCEPT ![c] = "unloading_model"]
    /\ model_loaded' = FALSE
    /\ IF synthesis_outcome[c] = "success"
       THEN response' = [response EXCEPT ![c] = "ok"]
       ELSE response' = [response EXCEPT ![c] = "error"]
    /\ pc' = [pc EXCEPT ![c] = "ReleaseMutex"]
    /\ UNCHANGED << semaphore, mutex_holder, request_type, synthesis_outcome >>

ReleaseMutex(c) ==
    /\ pc[c] = "ReleaseMutex"
    /\ mutex_holder' = "nobody"
    /\ client_state' = [client_state EXCEPT ![c] = "responding"]
    /\ pc' = [pc EXCEPT ![c] = "SendResponse"]
    /\ UNCHANGED << semaphore, model_loaded, request_type,
                    synthesis_outcome, response >>

SendResponse(c) ==
    /\ pc[c] = "SendResponse"
    /\ client_state' = [client_state EXCEPT ![c] = "done"]
    /\ semaphore' = semaphore + 1
    /\ pc' = [pc EXCEPT ![c] = "Done"]
    /\ UNCHANGED << mutex_holder, model_loaded, request_type, synthesis_outcome, response >>

ClientDisconnect(c) ==
    /\ pc[c] \in {"ChooseRequest", "AcquireMutex", "LoadModel",
                  "Synthesize", "UnloadModel", "ReleaseMutex", "SendResponse"}
    /\ client_state[c] \in {"has_permit", "waiting_mutex", "loading_model",
                            "synthesizing", "unloading_model", "responding"}
    /\ client_state' = [client_state EXCEPT ![c] = "aborted"]
    /\ response' = response
    /\ semaphore' = semaphore + 1
    /\ IF mutex_holder = c
       THEN /\ mutex_holder' = "nobody"
            /\ model_loaded' = FALSE
       ELSE /\ UNCHANGED << mutex_holder, model_loaded >>
    /\ pc' = [pc EXCEPT ![c] = "Done"]
    /\ UNCHANGED << request_type, synthesis_outcome >>

\* ================================================================
\* Specification
\* ================================================================

Terminated ==
    \A c \in Clients: pc[c] = "Done"

Next ==
    \/ (\E c \in Clients:
            \/ AcquirePermit(c)
            \/ ChooseRequest(c)
            \/ AcquireMutex(c)
            \/ LoadModel(c)
            \/ Synthesize(c)
            \/ UnloadModel(c)
            \/ ReleaseMutex(c)
            \/ SendResponse(c)
            \/ ClientDisconnect(c))
    \/ (Terminated /\ UNCHANGED vars)

Fairness ==
    \A c \in Clients: WF_vars(
        \/ AcquirePermit(c)
        \/ ChooseRequest(c)
        \/ AcquireMutex(c)
        \/ LoadModel(c)
        \/ Synthesize(c)
        \/ UnloadModel(c)
        \/ ReleaseMutex(c)
        \/ SendResponse(c)
        \/ ClientDisconnect(c)
    )

Spec == Init /\ [][Next]_vars /\ Fairness

\* ================================================================
\* Liveness
\* ================================================================

ClientTermination ==
    \A c \in Clients: <>(pc[c] = "Done")

\* ================================================================
\* Symmetry
\* ================================================================

ClientSymmetry == Permutations(Clients)

=============================================================================
