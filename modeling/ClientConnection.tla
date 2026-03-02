------------------------------ MODULE ClientConnection ------------------------------
(***************************************************************************)
(* Models client connection with auto-start, backoff retry, and            *)
(* transient connection failures even while daemon state is ready.          *)
(***************************************************************************)

EXTENDS Integers, TLC

CONSTANTS
    MAX_ATTEMPTS,
    INITIAL_DELAY,
    MAX_DELAY

ASSUME MAX_ATTEMPTS \in Nat /\ MAX_ATTEMPTS > 0
ASSUME INITIAL_DELAY \in Nat /\ INITIAL_DELAY > 0
ASSUME MAX_DELAY \in Nat /\ MAX_DELAY >= INITIAL_DELAY

VARIABLES daemon_state, models_available, client_phase, attempt, delay,
          last_connect_result, pc

vars == << daemon_state, models_available, client_phase, attempt, delay,
           last_connect_result, pc >>

TypeOK ==
    /\ daemon_state \in {"not_running", "starting", "ready", "crashed"}
    /\ models_available \in BOOLEAN
    /\ client_phase \in {"initial_connect", "check_models", "start_daemon",
                          "grace_wait", "retry_loop", "final_connect",
                          "connected", "failed"}
    /\ attempt \in 0..MAX_ATTEMPTS
    /\ delay \in INITIAL_DELAY..MAX_DELAY
    /\ last_connect_result \in {"none", "ok", "refused"}

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

LastConnectResultConsistency ==
    /\ (client_phase = "connected" => last_connect_result = "ok")
    /\ (last_connect_result = "ok" /\ pc["client"] # "Done" => daemon_state = "ready")

Init ==
    /\ daemon_state = "not_running"
    /\ models_available \in BOOLEAN
    /\ client_phase = "initial_connect"
    /\ attempt = 0
    /\ delay = INITIAL_DELAY
    /\ last_connect_result = "none"
    /\ pc = [p \in {"client", "env"} |->
                IF p = "client" THEN "InitialConnect" ELSE "EnvironmentLoop"]

InitialConnect ==
    /\ pc["client"] = "InitialConnect"
    /\ IF daemon_state = "ready"
       THEN /\ \/ /\ client_phase' = "connected"
                  /\ last_connect_result' = "ok"
                  /\ pc' = [pc EXCEPT !["client"] = "Done"]
               \/ /\ client_phase' = "check_models"
                  /\ last_connect_result' = "refused"
                  /\ pc' = [pc EXCEPT !["client"] = "CheckModels"]
       ELSE /\ client_phase' = "check_models"
            /\ last_connect_result' = "none"
            /\ pc' = [pc EXCEPT !["client"] = "CheckModels"]
    /\ UNCHANGED << daemon_state, models_available, attempt, delay >>

CheckModels ==
    /\ pc["client"] = "CheckModels"
    /\ IF ~models_available
       THEN /\ client_phase' = "failed"
            /\ pc' = [pc EXCEPT !["client"] = "Done"]
       ELSE /\ client_phase' = "start_daemon"
            /\ pc' = [pc EXCEPT !["client"] = "StartDaemon"]
    /\ UNCHANGED << daemon_state, models_available, attempt, delay, last_connect_result >>

StartDaemon ==
    /\ pc["client"] = "StartDaemon"
    /\ IF daemon_state = "not_running"
       THEN daemon_state' = "starting"
       ELSE UNCHANGED daemon_state
    /\ client_phase' = "grace_wait"
    /\ pc' = [pc EXCEPT !["client"] = "GraceWait"]
    /\ UNCHANGED << models_available, attempt, delay, last_connect_result >>

GraceWait ==
    /\ pc["client"] = "GraceWait"
    /\ client_phase' = "retry_loop"
    /\ attempt' = 0
    /\ delay' = INITIAL_DELAY
    /\ pc' = [pc EXCEPT !["client"] = "RetryLoop"]
    /\ UNCHANGED << daemon_state, models_available, last_connect_result >>

RetryLoop ==
    /\ pc["client"] = "RetryLoop"
    /\ IF attempt >= MAX_ATTEMPTS
       THEN /\ client_phase' = "final_connect"
            /\ pc' = [pc EXCEPT !["client"] = "FinalConnect"]
            /\ UNCHANGED << daemon_state, models_available, attempt, delay, last_connect_result >>
       ELSE IF daemon_state = "ready"
            THEN /\ \/ /\ client_phase' = "connected"
                       /\ last_connect_result' = "ok"
                       /\ pc' = [pc EXCEPT !["client"] = "Done"]
                       /\ UNCHANGED << daemon_state, models_available, attempt, delay >>
                    \/ /\ attempt' = attempt + 1
                       /\ IF attempt + 1 < MAX_ATTEMPTS
                          THEN IF delay * 2 <= MAX_DELAY
                               THEN delay' = delay * 2
                               ELSE delay' = MAX_DELAY
                          ELSE UNCHANGED delay
                       /\ client_phase' = "retry_loop"
                       /\ last_connect_result' = "refused"
                       /\ pc' = [pc EXCEPT !["client"] = "RetryLoop"]
                       /\ UNCHANGED << daemon_state, models_available >>
            ELSE /\ attempt' = attempt + 1
                 /\ IF attempt + 1 < MAX_ATTEMPTS
                    THEN IF delay * 2 <= MAX_DELAY
                         THEN delay' = delay * 2
                         ELSE delay' = MAX_DELAY
                    ELSE UNCHANGED delay
                 /\ client_phase' = "retry_loop"
                 /\ last_connect_result' = "none"
                 /\ pc' = [pc EXCEPT !["client"] = "RetryLoop"]
                 /\ UNCHANGED << daemon_state, models_available >>

FinalConnect ==
    /\ pc["client"] = "FinalConnect"
    /\ IF daemon_state = "ready"
       THEN /\ \/ /\ client_phase' = "connected"
                  /\ last_connect_result' = "ok"
               \/ /\ client_phase' = "failed"
                  /\ last_connect_result' = "refused"
       ELSE /\ client_phase' = "failed"
            /\ last_connect_result' = "none"
    /\ pc' = [pc EXCEPT !["client"] = "Done"]
    /\ UNCHANGED << daemon_state, models_available, attempt, delay >>

EnvironmentLoop ==
    /\ pc["env"] = "EnvironmentLoop"
    /\ \/ (daemon_state = "not_running" /\ daemon_state' \in {"not_running", "starting"})
       \/ (daemon_state = "starting" /\ daemon_state' \in {"starting", "ready", "crashed"})
       \/ (daemon_state = "ready" /\ daemon_state' \in {"ready", "crashed"})
       \/ (daemon_state = "crashed" /\ daemon_state' \in {"crashed", "not_running"})
    /\ models_available' \in BOOLEAN
    /\ client_phase' = client_phase
    /\ UNCHANGED << attempt, delay, last_connect_result >>
    /\ pc' = [pc EXCEPT !["env"] = "EnvironmentLoop"]

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

EventualTermination ==
    <>(client_phase \in {"connected", "failed"})

=============================================================================
