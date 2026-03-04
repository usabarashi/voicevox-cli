---------------------------------- MODULE Socket ----------------------------------
(***************************************************************************)
(* Minimal socket lifecycle model for daemon IPC endpoint.                 *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES socketState, retryCount

vars == << socketState, retryCount >>

TypeOK ==
    /\ socketState \in {"SocketAbsent", "Binding", "SocketReady", "PermissionDenied"}
    /\ retryCount \in 0..MAX_RETRY

ReadyIsBounded ==
    socketState = "SocketReady" => retryCount <= MAX_RETRY

Init ==
    /\ socketState = "SocketAbsent"
    /\ retryCount = 0

StartBind ==
    /\ socketState \in {"SocketAbsent", "PermissionDenied"}
    /\ socketState' = "Binding"
    /\ UNCHANGED retryCount

BindOk ==
    /\ socketState = "Binding"
    /\ socketState' = "SocketReady"
    /\ UNCHANGED retryCount

BindPermissionDenied ==
    /\ socketState = "Binding"
    /\ socketState' = "PermissionDenied"
    /\ retryCount' = IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount

SocketDrop ==
    /\ socketState = "SocketReady"
    /\ socketState' = "SocketAbsent"
    /\ UNCHANGED retryCount

RetryBind ==
    /\ socketState = "PermissionDenied"
    /\ retryCount < MAX_RETRY
    /\ socketState' = "Binding"
    /\ UNCHANGED retryCount

Stutter ==
    UNCHANGED vars

Next ==
    \/ StartBind
    \/ BindOk
    \/ BindPermissionDenied
    \/ SocketDrop
    \/ RetryBind
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

=============================================================================
