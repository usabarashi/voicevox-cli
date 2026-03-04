------------------------------- MODULE SynthesisParallel ----------------------------
(***************************************************************************)
(* Two-job parallel synthesis model with queue/worker/cancel interactions. *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES daemonReady, workerState,
          j1State, j2State,
          j1Retry, j2Retry

vars == << daemonReady, workerState, j1State, j2State, j1Retry, j2Retry >>

TypeOK ==
    /\ daemonReady \in BOOLEAN
    /\ workerState \in {"Idle", "BusyJ1", "BusyJ2"}
    /\ j1State \in {"Idle", "Queued", "Synthesizing", "Done", "Failed", "Canceled"}
    /\ j2State \in {"Idle", "Queued", "Synthesizing", "Done", "Failed", "Canceled"}
    /\ j1Retry \in 0..MAX_RETRY
    /\ j2Retry \in 0..MAX_RETRY

AtMostOneSynthesizing ==
    ~(j1State = "Synthesizing" /\ j2State = "Synthesizing")

WorkerMatchesSynthesis ==
    /\ (workerState = "BusyJ1") => j1State = "Synthesizing"
    /\ (workerState = "BusyJ2") => j2State = "Synthesizing"
    /\ (workerState = "Idle") =>
        /\ j1State # "Synthesizing"
        /\ j2State # "Synthesizing"

Init ==
    /\ daemonReady = FALSE
    /\ workerState = "Idle"
    /\ j1State = "Idle"
    /\ j2State = "Idle"
    /\ j1Retry = 0
    /\ j2Retry = 0

DaemonUp ==
    /\ ~daemonReady
    /\ daemonReady' = TRUE
    /\ UNCHANGED << workerState, j1State, j2State, j1Retry, j2Retry >>

DaemonDown ==
    /\ daemonReady
    /\ daemonReady' = FALSE
    /\ workerState' = "Idle"
    /\ j1State' = IF j1State \in {"Queued", "Synthesizing"} THEN "Idle" ELSE j1State
    /\ j2State' = IF j2State \in {"Queued", "Synthesizing"} THEN "Idle" ELSE j2State
    /\ UNCHANGED << j1Retry, j2Retry >>

EnqueueJ1 ==
    /\ daemonReady /\ j1State = "Idle"
    /\ j1State' = "Queued"
    /\ UNCHANGED << daemonReady, workerState, j2State, j1Retry, j2Retry >>

EnqueueJ2 ==
    /\ daemonReady /\ j2State = "Idle"
    /\ j2State' = "Queued"
    /\ UNCHANGED << daemonReady, workerState, j1State, j1Retry, j2Retry >>

StartJ1 ==
    /\ daemonReady /\ workerState = "Idle" /\ j1State = "Queued"
    /\ j1State' = "Synthesizing"
    /\ workerState' = "BusyJ1"
    /\ UNCHANGED << daemonReady, j2State, j1Retry, j2Retry >>

StartJ2 ==
    /\ daemonReady /\ workerState = "Idle" /\ j2State = "Queued"
    /\ j2State' = "Synthesizing"
    /\ workerState' = "BusyJ2"
    /\ UNCHANGED << daemonReady, j1State, j1Retry, j2Retry >>

FinishJ1 ==
    /\ workerState = "BusyJ1" /\ j1State = "Synthesizing"
    /\ j1State' = "Done"
    /\ workerState' = "Idle"
    /\ UNCHANGED << daemonReady, j2State, j1Retry, j2Retry >>

FinishJ2 ==
    /\ workerState = "BusyJ2" /\ j2State = "Synthesizing"
    /\ j2State' = "Done"
    /\ workerState' = "Idle"
    /\ UNCHANGED << daemonReady, j1State, j1Retry, j2Retry >>

FailJ1 ==
    /\ workerState = "BusyJ1" /\ j1State = "Synthesizing"
    /\ j1State' = IF j1Retry < MAX_RETRY THEN "Queued" ELSE "Failed"
    /\ j1Retry' = IF j1Retry < MAX_RETRY THEN j1Retry + 1 ELSE j1Retry
    /\ workerState' = "Idle"
    /\ UNCHANGED << daemonReady, j2State, j2Retry >>

FailJ2 ==
    /\ workerState = "BusyJ2" /\ j2State = "Synthesizing"
    /\ j2State' = IF j2Retry < MAX_RETRY THEN "Queued" ELSE "Failed"
    /\ j2Retry' = IF j2Retry < MAX_RETRY THEN j2Retry + 1 ELSE j2Retry
    /\ workerState' = "Idle"
    /\ UNCHANGED << daemonReady, j1State, j1Retry >>

CancelJ1 ==
    /\ j1State \in {"Queued", "Synthesizing"}
    /\ j1State' = "Canceled"
    /\ workerState' = IF workerState = "BusyJ1" THEN "Idle" ELSE workerState
    /\ UNCHANGED << daemonReady, j2State, j1Retry, j2Retry >>

CancelJ2 ==
    /\ j2State \in {"Queued", "Synthesizing"}
    /\ j2State' = "Canceled"
    /\ workerState' = IF workerState = "BusyJ2" THEN "Idle" ELSE workerState
    /\ UNCHANGED << daemonReady, j1State, j1Retry, j2Retry >>

ResetJ1 ==
    /\ j1State \in {"Done", "Failed", "Canceled"}
    /\ j1State' = "Idle"
    /\ j1Retry' = 0
    /\ UNCHANGED << daemonReady, workerState, j2State, j2Retry >>

ResetJ2 ==
    /\ j2State \in {"Done", "Failed", "Canceled"}
    /\ j2State' = "Idle"
    /\ j2Retry' = 0
    /\ UNCHANGED << daemonReady, workerState, j1State, j1Retry >>

Stutter ==
    UNCHANGED vars

Next ==
    \/ DaemonUp
    \/ DaemonDown
    \/ EnqueueJ1
    \/ EnqueueJ2
    \/ StartJ1
    \/ StartJ2
    \/ FinishJ1
    \/ FinishJ2
    \/ FailJ1
    \/ FailJ2
    \/ CancelJ1
    \/ CancelJ2
    \/ ResetJ1
    \/ ResetJ2
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

ProgressNext ==
    \/ DaemonUp
    \/ DaemonDown
    \/ EnqueueJ1
    \/ EnqueueJ2
    \/ StartJ1
    \/ StartJ2
    \/ FinishJ1
    \/ FinishJ2
    \/ FailJ1
    \/ FailJ2
    \/ CancelJ1
    \/ CancelJ2
    \/ ResetJ1
    \/ ResetJ2

ProgressSpec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(FinishJ1 \/ FailJ1 \/ CancelJ1 \/ DaemonDown)
    /\ WF_vars(FinishJ2 \/ FailJ2 \/ CancelJ2 \/ DaemonDown)

EventuallyLeavesBusyWorker ==
    [](workerState # "Idle" => <>(workerState = "Idle"))

=============================================================================
