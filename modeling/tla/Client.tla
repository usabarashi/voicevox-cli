---------------------------------- MODULE Client ----------------------------------
(***************************************************************************)
(* Minimal client-side connection/retry model.                             *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_ATTEMPTS
ASSUME MAX_ATTEMPTS \in Nat

VARIABLES daemonState, clientState, attempt

vars == << daemonState, clientState, attempt >>

TypeOK ==
    /\ daemonState \in {"DaemonDown", "Ready"}
    /\ clientState \in {"Idle", "Connecting", "Connected", "Failed"}
    /\ attempt \in 0..MAX_ATTEMPTS

ConnectedImpliesDaemonReady ==
    clientState = "Connected" => daemonState = "Ready"

Init ==
    /\ daemonState = "DaemonDown"
    /\ clientState = "Idle"
    /\ attempt = 0

StartConnect ==
    /\ clientState = "Idle"
    /\ clientState' = "Connecting"
    /\ UNCHANGED << daemonState, attempt >>

ConnectOk ==
    /\ clientState = "Connecting"
    /\ daemonState = "Ready"
    /\ clientState' = "Connected"
    /\ UNCHANGED << daemonState, attempt >>

ConnectRetry ==
    /\ clientState = "Connecting"
    /\ daemonState = "DaemonDown"
    /\ attempt < MAX_ATTEMPTS
    /\ clientState' = "Idle"
    /\ attempt' = attempt + 1
    /\ UNCHANGED daemonState

ConnectFail ==
    /\ clientState = "Connecting"
    /\ daemonState = "DaemonDown"
    /\ attempt = MAX_ATTEMPTS
    /\ clientState' = "Failed"
    /\ UNCHANGED << daemonState, attempt >>

DaemonReady ==
    /\ daemonState = "DaemonDown"
    /\ daemonState' = "Ready"
    /\ UNCHANGED << clientState, attempt >>

DaemonDown ==
    /\ daemonState = "Ready"
    /\ daemonState' = "DaemonDown"
    /\ clientState' = IF clientState = "Connected" THEN "Idle" ELSE clientState
    /\ UNCHANGED attempt

Next ==
    \/ StartConnect
    \/ ConnectOk
    \/ ConnectRetry
    \/ ConnectFail
    \/ DaemonReady
    \/ DaemonDown

Spec ==
    Init /\ [][Next]_vars

=============================================================================
