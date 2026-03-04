---------------------------------- MODULE Daemon ----------------------------------
(***************************************************************************)
(* Minimal startup/recovery model focused on state-space control.          *)
(* The model intentionally abstracts away PID, stderr text, and file-path  *)
(* detail into a small set of state transitions.                           *)
(***************************************************************************)

EXTENDS Naturals, TLC

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES daemonState, socketState, reqState, retryCount

vars == << daemonState, socketState, reqState, retryCount >>

TypeOK ==
    /\ daemonState \in {"DaemonDown", "Starting", "AlreadyRunning", "Ready", "Recovering"}
    /\ socketState \in {"SocketAbsent", "SocketReady"}
    /\ reqState \in {"Idle", "Busy"}
    /\ retryCount \in 0..MAX_RETRY

SocketImpliesReady ==
    socketState = "SocketReady" => daemonState = "Ready"

BusyImpliesReady ==
    reqState = "Busy" => /\ daemonState = "Ready"
                         /\ socketState = "SocketReady"

RetryBounded ==
    retryCount <= MAX_RETRY

AlreadyRunningNotBusy ==
    daemonState = "AlreadyRunning" => reqState = "Idle"

Init ==
    /\ daemonState = "DaemonDown"
    /\ socketState = "SocketAbsent"
    /\ reqState = "Idle"
    /\ retryCount = 0

StartDaemon ==
    /\ daemonState = "DaemonDown"
    /\ daemonState' = "Starting"
    /\ UNCHANGED << socketState, reqState, retryCount >>

DaemonReady ==
    /\ daemonState = "Starting"
    /\ daemonState' = "Ready"
    /\ socketState' = "SocketReady"
    /\ UNCHANGED << reqState, retryCount >>

AlreadyRunningDetected ==
    /\ daemonState = "Starting"
    /\ daemonState' = "AlreadyRunning"
    /\ UNCHANGED << socketState, reqState, retryCount >>

AlreadyRunningResponsive ==
    /\ daemonState = "AlreadyRunning"
    /\ daemonState' = "Ready"
    /\ socketState' = "SocketReady"
    /\ UNCHANGED << reqState, retryCount >>

AlreadyRunningUnresponsive ==
    /\ daemonState = "AlreadyRunning"
    /\ daemonState' = "Recovering"
    /\ socketState' = "SocketAbsent"
    /\ reqState' = "Idle"
    /\ retryCount' = IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount

DaemonFail ==
    /\ daemonState \in {"Starting", "Recovering"}
    /\ daemonState' = "Recovering"
    /\ socketState' = "SocketAbsent"
    /\ reqState' = "Idle"
    /\ retryCount' = IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount

CrashFromReady ==
    /\ daemonState = "Ready"
    /\ daemonState' = "Recovering"
    /\ socketState' = "SocketAbsent"
    /\ reqState' = "Idle"
    /\ retryCount' = IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount

Recover ==
    /\ daemonState = "Recovering"
    /\ retryCount < MAX_RETRY
    /\ daemonState' = "Starting"
    /\ UNCHANGED << socketState, reqState, retryCount >>

GiveUp ==
    /\ daemonState = "Recovering"
    /\ retryCount = MAX_RETRY
    /\ daemonState' = "DaemonDown"
    /\ socketState' = "SocketAbsent"
    /\ reqState' = "Idle"
    /\ UNCHANGED retryCount

AcceptReq ==
    /\ daemonState = "Ready"
    /\ socketState = "SocketReady"
    /\ reqState = "Idle"
    /\ reqState' = "Busy"
    /\ UNCHANGED << daemonState, socketState, retryCount >>

FinishReq ==
    /\ reqState = "Busy"
    /\ reqState' = "Idle"
    /\ UNCHANGED << daemonState, socketState, retryCount >>

Next ==
    \/ StartDaemon
    \/ DaemonReady
    \/ AlreadyRunningDetected
    \/ AlreadyRunningResponsive
    \/ AlreadyRunningUnresponsive
    \/ DaemonFail
    \/ CrashFromReady
    \/ Recover
    \/ GiveUp
    \/ AcceptReq
    \/ FinishReq

Spec ==
    Init /\ [][Next]_vars

RecoveryPathExists ==
    []((daemonState = "Recovering" /\ retryCount < MAX_RETRY) => <> (daemonState = "Starting"))

=============================================================================
