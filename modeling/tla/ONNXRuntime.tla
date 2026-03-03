------------------------------- MODULE ONNXRuntime -------------------------------
(***************************************************************************)
(* Minimal ONNX Runtime lifecycle model (loader/setup state).              *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES runtimeState, retryCount

vars == << runtimeState, retryCount >>

TypeOK ==
    /\ runtimeState \in {"Missing", "Loading", "Ready", "Failed"}
    /\ retryCount \in 0..MAX_RETRY

ReadyHasNoPendingRetry ==
    runtimeState = "Ready" => retryCount <= MAX_RETRY

Init ==
    /\ runtimeState = "Missing"
    /\ retryCount = 0

BeginLoad ==
    /\ runtimeState \in {"Missing", "Failed"}
    /\ runtimeState' = "Loading"
    /\ UNCHANGED retryCount

LoadOk ==
    /\ runtimeState = "Loading"
    /\ runtimeState' = "Ready"
    /\ UNCHANGED retryCount

LoadFail ==
    /\ runtimeState = "Loading"
    /\ runtimeState' = "Failed"
    /\ retryCount' = IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount

RetryLoad ==
    /\ runtimeState = "Failed"
    /\ retryCount < MAX_RETRY
    /\ runtimeState' = "Loading"
    /\ UNCHANGED retryCount

GiveUp ==
    /\ runtimeState = "Failed"
    /\ retryCount = MAX_RETRY
    /\ UNCHANGED << runtimeState, retryCount >>

Stutter ==
    UNCHANGED vars

Next ==
    \/ BeginLoad
    \/ LoadOk
    \/ LoadFail
    \/ RetryLoad
    \/ GiveUp
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

=============================================================================
