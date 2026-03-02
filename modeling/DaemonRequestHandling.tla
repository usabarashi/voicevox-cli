--------------------------- MODULE DaemonRequestHandling ---------------------------
(***************************************************************************)
(* Models daemon request handling with bounded repeated requests per client *)
(* and disconnection from both permit-wait and in-flight states.           *)
(***************************************************************************)

EXTENDS Integers, FiniteSets, TLC

CONSTANTS
    MAX_CLIENTS,
    MAX_REQUESTS,
    Clients

ASSUME MAX_CLIENTS \in Nat /\ MAX_CLIENTS > 0
ASSUME MAX_REQUESTS \in Nat /\ MAX_REQUESTS > 0
ASSUME Clients # {} /\ IsFiniteSet(Clients)

RequestTypes == {"Synthesize", "ListSpeakers", "ListModels"}
SynthesisOutcomes == {"success", "load_failed", "synthesis_failed", "invalid_target"}

VARIABLES semaphore, mutex_holder, model_loaded,
          client_state, request_type, synthesis_outcome, response,
          completed_requests, pc

vars == << semaphore, mutex_holder, model_loaded,
           client_state, request_type, synthesis_outcome, response,
           completed_requests, pc >>

TypeOK ==
    /\ semaphore \in 0..MAX_CLIENTS
    /\ mutex_holder \in Clients \cup {"nobody"}
    /\ model_loaded \in BOOLEAN
    /\ \A c \in Clients:
        /\ client_state[c] \in {"waiting_permit", "has_permit", "waiting_mutex",
               "loading_model", "synthesizing", "unloading_model",
               "responding", "done", "aborted"}
        /\ request_type[c] \in RequestTypes
        /\ synthesis_outcome[c] \in SynthesisOutcomes
        /\ response[c] \in {"none", "ok", "error"}
        /\ completed_requests[c] \in 0..MAX_REQUESTS

ActiveHandlers ==
    {c \in Clients: client_state[c] \in
        {"has_permit", "waiting_mutex", "loading_model", "synthesizing", "unloading_model", "responding"}}

MutexExclusion ==
    Cardinality({c \in Clients:
        pc[c] \in {"LoadModel", "Synthesize", "UnloadModel", "ReleaseMutex"}
    }) <= 1

SemaphoreBound ==
    Cardinality(ActiveHandlers) <= MAX_CLIENTS

SemaphoreConsistency ==
    semaphore = MAX_CLIENTS - Cardinality(ActiveHandlers)

ModelCleanup ==
    \A c \in Clients:
        (pc[c] = "ReleaseMutex") => ~model_loaded

MutexConsistency ==
    \A c \in Clients:
        (mutex_holder = c) <=>
        (pc[c] \in {"LoadModel", "Synthesize", "UnloadModel", "ReleaseMutex"})

ModelOnlyUnderMutex ==
    model_loaded => (mutex_holder # "nobody")

ClientStatePcConsistency ==
    \A c \in Clients:
        /\ (pc[c] = "AcquirePermit" => client_state[c] \in {"waiting_permit", "done", "aborted"})
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

CompletedRequestsBounded ==
    \A c \in Clients: completed_requests[c] <= MAX_REQUESTS

Init ==
    /\ semaphore = MAX_CLIENTS
    /\ mutex_holder = "nobody"
    /\ model_loaded = FALSE
    /\ client_state = [c \in Clients |-> "waiting_permit"]
    /\ request_type = [c \in Clients |-> "Synthesize"]
    /\ synthesis_outcome = [c \in Clients |-> "success"]
    /\ response = [c \in Clients |-> "none"]
    /\ completed_requests = [c \in Clients |-> 0]
    /\ pc = [c \in Clients |-> "AcquirePermit"]

AcquirePermit(c) ==
    /\ pc[c] = "AcquirePermit"
    /\ client_state[c] = "waiting_permit"
    /\ completed_requests[c] < MAX_REQUESTS
    /\ semaphore > 0
    /\ semaphore' = semaphore - 1
    /\ client_state' = [client_state EXCEPT ![c] = "has_permit"]
    /\ pc' = [pc EXCEPT ![c] = "ChooseRequest"]
    /\ UNCHANGED << mutex_holder, model_loaded, request_type,
                    synthesis_outcome, response, completed_requests >>

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
    /\ UNCHANGED << semaphore, mutex_holder, model_loaded,
                    synthesis_outcome, completed_requests >>

AcquireMutex(c) ==
    /\ pc[c] = "AcquireMutex"
    /\ mutex_holder = "nobody"
    /\ mutex_holder' = c
    /\ pc' = [pc EXCEPT ![c] = "LoadModel"]
    /\ UNCHANGED << semaphore, model_loaded, client_state, request_type,
                    synthesis_outcome, response, completed_requests >>

LoadModel(c) ==
    /\ pc[c] = "LoadModel"
    /\ \E outcome \in {"success", "load_failed", "invalid_target"}:
        /\ synthesis_outcome' = [synthesis_outcome EXCEPT ![c] = outcome]
        /\ IF outcome = "success"
           THEN /\ client_state' = [client_state EXCEPT ![c] = "loading_model"]
                /\ model_loaded' = TRUE
                /\ pc' = [pc EXCEPT ![c] = "Synthesize"]
                /\ UNCHANGED response
           ELSE /\ client_state' = [client_state EXCEPT ![c] = "responding"]
                /\ response' = [response EXCEPT ![c] = "error"]
                /\ pc' = [pc EXCEPT ![c] = "ReleaseMutex"]
                /\ UNCHANGED model_loaded
    /\ UNCHANGED << semaphore, mutex_holder, request_type, completed_requests >>

Synthesize(c) ==
    /\ pc[c] = "Synthesize"
    /\ client_state' = [client_state EXCEPT ![c] = "synthesizing"]
    /\ \E outcome \in {"success", "synthesis_failed"}:
        synthesis_outcome' = [synthesis_outcome EXCEPT ![c] = outcome]
    /\ pc' = [pc EXCEPT ![c] = "UnloadModel"]
    /\ UNCHANGED << semaphore, mutex_holder, model_loaded, request_type,
                    response, completed_requests >>

UnloadModel(c) ==
    /\ pc[c] = "UnloadModel"
    /\ client_state' = [client_state EXCEPT ![c] = "unloading_model"]
    /\ model_loaded' = FALSE
    /\ IF synthesis_outcome[c] = "success"
       THEN response' = [response EXCEPT ![c] = "ok"]
       ELSE response' = [response EXCEPT ![c] = "error"]
    /\ pc' = [pc EXCEPT ![c] = "ReleaseMutex"]
    /\ UNCHANGED << semaphore, mutex_holder, request_type,
                    synthesis_outcome, completed_requests >>

ReleaseMutex(c) ==
    /\ pc[c] = "ReleaseMutex"
    /\ mutex_holder' = "nobody"
    /\ client_state' = [client_state EXCEPT ![c] = "responding"]
    /\ pc' = [pc EXCEPT ![c] = "SendResponse"]
    /\ UNCHANGED << semaphore, model_loaded, request_type,
                    synthesis_outcome, response, completed_requests >>

SendResponse(c) ==
    /\ pc[c] = "SendResponse"
    /\ completed_requests[c] < MAX_REQUESTS
    /\ completed_requests' = [completed_requests EXCEPT ![c] = @ + 1]
    /\ semaphore' = semaphore + 1
    /\ IF completed_requests[c] + 1 < MAX_REQUESTS
       THEN /\ client_state' = [client_state EXCEPT ![c] = "waiting_permit"]
            /\ response' = [response EXCEPT ![c] = "none"]
            /\ pc' = [pc EXCEPT ![c] = "AcquirePermit"]
       ELSE /\ client_state' = [client_state EXCEPT ![c] = "done"]
            /\ UNCHANGED response
            /\ pc' = [pc EXCEPT ![c] = "Done"]
    /\ UNCHANGED << mutex_holder, model_loaded, request_type, synthesis_outcome >>

ClientDisconnectWaitingPermit(c) ==
    /\ pc[c] = "AcquirePermit"
    /\ client_state[c] = "waiting_permit"
    /\ client_state' = [client_state EXCEPT ![c] = "aborted"]
    /\ response' = response
    /\ pc' = [pc EXCEPT ![c] = "Done"]
    /\ UNCHANGED << semaphore, mutex_holder, model_loaded,
                    request_type, synthesis_outcome, completed_requests >>

ClientDisconnectActive(c) ==
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
    /\ UNCHANGED << request_type, synthesis_outcome, completed_requests >>

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
            \/ ClientDisconnectWaitingPermit(c)
            \/ ClientDisconnectActive(c))
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
    )

Spec == Init /\ [][Next]_vars /\ Fairness

ClientTermination ==
    \A c \in Clients: <>(pc[c] = "Done")

WaitingMutexEventuallyLeavesWait ==
    \A c \in Clients:
        [](pc[c] = "AcquireMutex" => <>(pc[c] # "AcquireMutex"))

WaitingPermitEventuallyLeavesWait ==
    \A c \in Clients:
        [](pc[c] = "AcquirePermit" /\ client_state[c] = "waiting_permit"
          => <>(pc[c] # "AcquirePermit"))

ClientSymmetry == Permutations(Clients)

=============================================================================
