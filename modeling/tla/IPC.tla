------------------------------------ MODULE IPC ------------------------------------
(***************************************************************************)
(* IPC request/response model with protocol-level failure branches.        *)
(* Covers: frame corruption, response mismatch, and timeout.              *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_TIMEOUTS
ASSUME MAX_TIMEOUTS \in Nat

VARIABLES connState, reqState, respState, ipcError, timeoutCount

vars == << connState, reqState, respState, ipcError, timeoutCount >>

TypeOK ==
    /\ connState \in {"Disconnected", "Connected"}
    /\ reqState \in {"Idle", "InFlight", "Done", "Failed"}
    /\ respState \in {"None", "Valid", "Corrupt", "Mismatched", "Timeout"}
    /\ ipcError \in {"None", "CorruptFrame", "ResponseMismatch", "ResponseTimeout"}
    /\ timeoutCount \in 0..MAX_TIMEOUTS

FailedImpliesError ==
    reqState = "Failed" => ipcError # "None"

DoneImpliesValidResponse ==
    reqState = "Done" => respState = "Valid"

Init ==
    /\ connState = "Disconnected"
    /\ reqState = "Idle"
    /\ respState = "None"
    /\ ipcError = "None"
    /\ timeoutCount = 0

Connect ==
    /\ connState = "Disconnected"
    /\ connState' = "Connected"
    /\ UNCHANGED << reqState, respState, ipcError, timeoutCount >>

Disconnect ==
    /\ connState = "Connected"
    /\ connState' = "Disconnected"
    /\ reqState' = "Idle"
    /\ respState' = "None"
    /\ ipcError' = "None"
    /\ UNCHANGED timeoutCount

SendRequest ==
    /\ connState = "Connected"
    /\ reqState = "Idle"
    /\ reqState' = "InFlight"
    /\ respState' = "None"
    /\ ipcError' = "None"
    /\ UNCHANGED << connState, timeoutCount >>

ReceiveValidResponse ==
    /\ connState = "Connected"
    /\ reqState = "InFlight"
    /\ reqState' = "Done"
    /\ respState' = "Valid"
    /\ ipcError' = "None"
    /\ UNCHANGED << connState, timeoutCount >>

CorruptFrame ==
    /\ connState = "Connected"
    /\ reqState = "InFlight"
    /\ reqState' = "Failed"
    /\ respState' = "Corrupt"
    /\ ipcError' = "CorruptFrame"
    /\ UNCHANGED << connState, timeoutCount >>

ResponseMismatch ==
    /\ connState = "Connected"
    /\ reqState = "InFlight"
    /\ reqState' = "Failed"
    /\ respState' = "Mismatched"
    /\ ipcError' = "ResponseMismatch"
    /\ UNCHANGED << connState, timeoutCount >>

ResponseTimeout ==
    /\ connState = "Connected"
    /\ reqState = "InFlight"
    /\ reqState' = "Failed"
    /\ respState' = "Timeout"
    /\ ipcError' = "ResponseTimeout"
    /\ timeoutCount' =
        IF timeoutCount < MAX_TIMEOUTS THEN timeoutCount + 1 ELSE timeoutCount
    /\ UNCHANGED connState

ResetRequest ==
    /\ reqState \in {"Done", "Failed"}
    /\ reqState' = "Idle"
    /\ respState' = "None"
    /\ ipcError' = "None"
    /\ UNCHANGED << connState, timeoutCount >>

Stutter ==
    UNCHANGED vars

Next ==
    \/ Connect
    \/ Disconnect
    \/ SendRequest
    \/ ReceiveValidResponse
    \/ CorruptFrame
    \/ ResponseMismatch
    \/ ResponseTimeout
    \/ ResetRequest
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

ProgressNext ==
    \/ Connect
    \/ Disconnect
    \/ SendRequest
    \/ ReceiveValidResponse
    \/ CorruptFrame
    \/ ResponseMismatch
    \/ ResponseTimeout
    \/ ResetRequest

ProgressSpec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(ReceiveValidResponse
               \/ CorruptFrame
               \/ ResponseMismatch
               \/ ResponseTimeout
               \/ Disconnect)

EventuallyLeavesInFlight ==
    [](reqState = "InFlight" => <>(reqState # "InFlight"))

=============================================================================
