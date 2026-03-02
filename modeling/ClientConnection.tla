------------------------------ MODULE ClientConnection ------------------------------
(***************************************************************************)
(* Models the VOICEVOX client connection protocol with automatic daemon    *)
(* startup and exponential backoff retry.                                  *)
(*                                                                         *)
(* Corresponding implementation:                                           *)
(*   src/client/daemon_client/launcher.rs  -- connect_or_start             *)
(*   src/daemon/bootstrap.rs              -- ensure_daemon_running         *)
(*   src/daemon/socket_probe.rs           -- wait_for_socket_ready_with_backoff *)
(*                                                                         *)
(* Processes:                                                              *)
(*   Client: InitialConnect -> CheckModels -> StartDaemon -> GraceWait ->  *)
(*           RetryLoop -> FinalConnect                                     *)
(*   Environment: non-deterministic daemon startup                         *)
(***************************************************************************)

EXTENDS Integers, TLC

CONSTANTS
    MAX_ATTEMPTS,
    INITIAL_DELAY,
    MAX_DELAY

ASSUME MAX_ATTEMPTS \in Nat /\ MAX_ATTEMPTS > 0
ASSUME INITIAL_DELAY \in Nat /\ INITIAL_DELAY > 0
ASSUME MAX_DELAY \in Nat /\ MAX_DELAY >= INITIAL_DELAY

\* ================================================================
\* Variables
\* ================================================================

VARIABLES daemon_state, models_available, client_phase, attempt, delay, pc

vars == << daemon_state, models_available, client_phase, attempt, delay, pc >>

\* ================================================================
\* Invariants
\* ================================================================

TypeOK ==
    /\ daemon_state \in {"not_running", "starting", "ready", "crashed"}
    /\ models_available \in BOOLEAN
    /\ client_phase \in {"initial_connect", "check_models", "start_daemon",
                          "grace_wait", "retry_loop", "final_connect",
                          "connected", "failed"}
    /\ attempt \in 0..MAX_ATTEMPTS
    /\ delay \in INITIAL_DELAY..MAX_DELAY

FinalConnectUsesMaxAttempts ==
    client_phase = "final_connect" => attempt = MAX_ATTEMPTS

ClientPcPhaseConsistency ==
    /\ (pc["client"] = "RetryLoop" => client_phase = "retry_loop")
    /\ (pc["client"] = "FinalConnect" => client_phase = "final_connect")
    /\ (pc["client"] = "Done" => client_phase \in {"connected", "failed"})

ConnectedImpliesReady ==
    (pc["client"] # "Done" /\ client_phase = "connected") => daemon_state = "ready"

TerminalPhaseStable ==
    pc["client"] = "Done" => client_phase \in {"connected", "failed"}

ConnectedIsTerminal ==
    client_phase = "connected" => pc["client"] = "Done"

FailedIsTerminal ==
    client_phase = "failed" => pc["client"] = "Done"

PreRetryBackoffInitialized ==
    client_phase \in {"initial_connect", "check_models", "start_daemon", "grace_wait"}
        => /\ attempt = 0
           /\ delay = INITIAL_DELAY

RetryLoopDelayDiscipline ==
    client_phase = "retry_loop"
        => delay =
            IF attempt = 0
            THEN INITIAL_DELAY
            ELSE IF INITIAL_DELAY * (2 ^ attempt) <= MAX_DELAY
                 THEN INITIAL_DELAY * (2 ^ attempt)
                 ELSE MAX_DELAY

\* ================================================================
\* Initial State
\* ================================================================

Init ==
    /\ daemon_state = "not_running"
    /\ models_available \in BOOLEAN
    /\ client_phase = "initial_connect"
    /\ attempt = 0
    /\ delay = INITIAL_DELAY
    /\ pc = [p \in {"client", "env"} |->
                IF p = "client" THEN "InitialConnect" ELSE "EnvironmentLoop"]

\* ================================================================
\* Client Actions
\* ================================================================

InitialConnect ==
    /\ pc["client"] = "InitialConnect"
    /\ IF daemon_state = "ready"
       THEN /\ client_phase' = "connected"
            /\ pc' = [pc EXCEPT !["client"] = "Done"]
       ELSE /\ client_phase' = "check_models"
            /\ pc' = [pc EXCEPT !["client"] = "CheckModels"]
    /\ UNCHANGED << daemon_state, models_available, attempt, delay >>

CheckModels ==
    /\ pc["client"] = "CheckModels"
    /\ IF ~models_available
       THEN /\ client_phase' = "failed"
            /\ pc' = [pc EXCEPT !["client"] = "Done"]
       ELSE /\ client_phase' = "start_daemon"
            /\ pc' = [pc EXCEPT !["client"] = "StartDaemon"]
    /\ UNCHANGED << daemon_state, models_available, attempt, delay >>

StartDaemon ==
    /\ pc["client"] = "StartDaemon"
    /\ IF daemon_state = "not_running"
       THEN daemon_state' = "starting"
       ELSE UNCHANGED daemon_state
    /\ client_phase' = "grace_wait"
    /\ pc' = [pc EXCEPT !["client"] = "GraceWait"]
    /\ UNCHANGED << models_available, attempt, delay >>

GraceWait ==
    /\ pc["client"] = "GraceWait"
    /\ client_phase' = "retry_loop"
    /\ attempt' = 0
    /\ delay' = INITIAL_DELAY
    /\ pc' = [pc EXCEPT !["client"] = "RetryLoop"]
    /\ UNCHANGED << daemon_state, models_available >>

RetryLoop ==
    /\ pc["client"] = "RetryLoop"
    /\ IF attempt >= MAX_ATTEMPTS
       THEN /\ client_phase' = "final_connect"
            /\ pc' = [pc EXCEPT !["client"] = "FinalConnect"]
            /\ UNCHANGED << daemon_state, models_available, attempt, delay >>
       ELSE IF daemon_state = "ready"
            THEN /\ client_phase' = "connected"
                 /\ pc' = [pc EXCEPT !["client"] = "Done"]
                 /\ UNCHANGED << daemon_state, models_available, attempt, delay >>
            ELSE /\ attempt' = attempt + 1
                 /\ IF attempt + 1 < MAX_ATTEMPTS
                    THEN IF delay * 2 <= MAX_DELAY
                         THEN delay' = delay * 2
                         ELSE delay' = MAX_DELAY
                    ELSE UNCHANGED delay
                 /\ UNCHANGED << daemon_state, models_available, client_phase >>
                 /\ pc' = [pc EXCEPT !["client"] = "RetryLoop"]

FinalConnect ==
    /\ pc["client"] = "FinalConnect"
    /\ IF daemon_state = "ready"
       THEN client_phase' = "connected"
       ELSE client_phase' = "failed"
    /\ pc' = [pc EXCEPT !["client"] = "Done"]
    /\ UNCHANGED << daemon_state, models_available, attempt, delay >>

\* ================================================================
\* Environment Actions
\* ================================================================

EnvironmentLoop ==
    /\ pc["env"] = "EnvironmentLoop"
    /\ \/ (daemon_state = "not_running" /\ daemon_state' \in {"not_running", "starting"})
       \/ (daemon_state = "starting" /\ daemon_state' \in {"starting", "ready", "crashed"})
       \/ (daemon_state = "ready" /\ daemon_state' \in {"ready", "crashed"})
       \/ (daemon_state = "crashed" /\ daemon_state' \in {"crashed", "not_running"})
    /\ models_available' \in BOOLEAN
    /\ client_phase' = client_phase
    /\ UNCHANGED << attempt, delay >>
    /\ pc' = [pc EXCEPT !["env"] = "EnvironmentLoop"]

\* ================================================================
\* Specification
\* ================================================================

Next ==
    \/ InitialConnect
    \/ CheckModels
    \/ StartDaemon
    \/ GraceWait
    \/ RetryLoop
    \/ FinalConnect
    \/ EnvironmentLoop

Fairness ==
    /\ WF_vars(InitialConnect)
    /\ WF_vars(CheckModels)
    /\ WF_vars(StartDaemon)
    /\ WF_vars(GraceWait)
    /\ WF_vars(RetryLoop)
    /\ WF_vars(FinalConnect)
    /\ WF_vars(EnvironmentLoop)

Spec == Init /\ [][Next]_vars /\ Fairness

\* ================================================================
\* Liveness
\* ================================================================

EventualTermination ==
    <>(client_phase \in {"connected", "failed"})

=============================================================================
