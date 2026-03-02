-------------------------------- MODULE Dictionary --------------------------------
(***************************************************************************)
(* Minimal dictionary lifecycle model (e.g. OpenJTalk dictionary).         *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES dictState, retryCount

vars == << dictState, retryCount >>

TypeOK ==
    /\ dictState \in {"Missing", "Loading", "Ready", "Failed"}
    /\ retryCount \in 0..MAX_RETRY

ReadyIsStable ==
    dictState = "Ready" => retryCount <= MAX_RETRY

Init ==
    /\ dictState = "Missing"
    /\ retryCount = 0

BeginLoad ==
    /\ dictState \in {"Missing", "Failed"}
    /\ dictState' = "Loading"
    /\ UNCHANGED retryCount

LoadOk ==
    /\ dictState = "Loading"
    /\ dictState' = "Ready"
    /\ UNCHANGED retryCount

LoadFail ==
    /\ dictState = "Loading"
    /\ dictState' = "Failed"
    /\ retryCount' = IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount

RetryLoad ==
    /\ dictState = "Failed"
    /\ retryCount < MAX_RETRY
    /\ dictState' = "Loading"
    /\ UNCHANGED retryCount

GiveUp ==
    /\ dictState = "Failed"
    /\ retryCount = MAX_RETRY
    /\ UNCHANGED << dictState, retryCount >>

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
