-------------------------------- MODULE Synthesis ---------------------------------
(***************************************************************************)
(* Minimal synthesis job lifecycle model with cancellation.                *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES daemonReady, synthState, retryCount, errorKind, cancelSource

vars == << daemonReady, synthState, retryCount, errorKind, cancelSource >>

TypeOK ==
    /\ daemonReady \in BOOLEAN
    /\ synthState \in {"Idle", "Queued", "Synthesizing", "Done", "Failed", "Canceled"}
    /\ retryCount \in 0..MAX_RETRY
    /\ errorKind \in {"None", "InvalidTarget", "SynthesisFailed"}
    /\ cancelSource \in {"None", "ExternalRequest", "ClientDisconnect", "WriteFailure", "Shutdown"}

TerminalStates ==
    synthState \in {"Done", "Failed", "Canceled"} => retryCount <= MAX_RETRY

SynthesisNeedsDaemon ==
    synthState = "Synthesizing" => daemonReady

CanceledHasSource ==
    synthState = "Canceled" => cancelSource # "None"

NonCanceledHasNoSource ==
    synthState # "Canceled" => cancelSource = "None"

Init ==
    /\ daemonReady = FALSE
    /\ synthState = "Idle"
    /\ retryCount = 0
    /\ errorKind = "None"
    /\ cancelSource = "None"

DaemonUp ==
    /\ ~daemonReady
    /\ daemonReady' = TRUE
    /\ UNCHANGED << synthState, retryCount, errorKind, cancelSource >>

DaemonDown ==
    /\ daemonReady
    /\ daemonReady' = FALSE
    /\ synthState' =
        IF synthState \in {"Queued", "Synthesizing"} THEN "Idle" ELSE synthState
    /\ errorKind' =
        IF synthState \in {"Queued", "Synthesizing"} THEN "None" ELSE errorKind
    /\ cancelSource' =
        IF synthState \in {"Queued", "Synthesizing"} THEN "None" ELSE cancelSource
    /\ UNCHANGED retryCount

Enqueue ==
    /\ daemonReady
    /\ synthState = "Idle"
    /\ synthState' = "Queued"
    /\ errorKind' = "None"
    /\ cancelSource' = "None"
    /\ UNCHANGED << daemonReady, retryCount >>

StartSynth ==
    /\ daemonReady
    /\ synthState = "Queued"
    /\ synthState' = "Synthesizing"
    /\ UNCHANGED << daemonReady, retryCount, errorKind, cancelSource >>

SynthOk ==
    /\ synthState = "Synthesizing"
    /\ synthState' = "Done"
    /\ errorKind' = "None"
    /\ cancelSource' = "None"
    /\ UNCHANGED << daemonReady, retryCount >>

SynthFail ==
    /\ synthState = "Synthesizing"
    /\ synthState' =
        IF retryCount < MAX_RETRY THEN "Queued" ELSE "Failed"
    /\ retryCount' =
        IF retryCount < MAX_RETRY THEN retryCount + 1 ELSE retryCount
    /\ errorKind' = "SynthesisFailed"
    /\ cancelSource' = "None"
    /\ UNCHANGED daemonReady

InvalidTargetFail ==
    /\ synthState = "Queued"
    /\ synthState' = "Failed"
    /\ errorKind' = "InvalidTarget"
    /\ cancelSource' = "None"
    /\ UNCHANGED << daemonReady, retryCount >>

CancelByExternalRequest ==
    /\ synthState \in {"Queued", "Synthesizing"}
    /\ synthState' = "Canceled"
    /\ cancelSource' = "ExternalRequest"
    /\ UNCHANGED << daemonReady, retryCount, errorKind >>

CancelByClientDisconnect ==
    /\ synthState \in {"Queued", "Synthesizing"}
    /\ synthState' = "Canceled"
    /\ cancelSource' = "ClientDisconnect"
    /\ UNCHANGED << daemonReady, retryCount, errorKind >>

CancelByWriteFailure ==
    /\ synthState \in {"Queued", "Synthesizing"}
    /\ synthState' = "Canceled"
    /\ cancelSource' = "WriteFailure"
    /\ UNCHANGED << daemonReady, retryCount, errorKind >>

CancelByShutdown ==
    /\ synthState \in {"Queued", "Synthesizing"}
    /\ synthState' = "Canceled"
    /\ cancelSource' = "Shutdown"
    /\ UNCHANGED << daemonReady, retryCount, errorKind >>

Reset ==
    /\ synthState \in {"Done", "Failed", "Canceled"}
    /\ synthState' = "Idle"
    /\ retryCount' = 0
    /\ errorKind' = "None"
    /\ cancelSource' = "None"
    /\ UNCHANGED daemonReady

InvalidTargetIsTerminalFailure ==
    errorKind = "InvalidTarget" => synthState = "Failed"

Stutter ==
    UNCHANGED vars

Next ==
    \/ DaemonUp
    \/ DaemonDown
    \/ Enqueue
    \/ StartSynth
    \/ SynthOk
    \/ SynthFail
    \/ InvalidTargetFail
    \/ CancelByExternalRequest
    \/ CancelByClientDisconnect
    \/ CancelByWriteFailure
    \/ CancelByShutdown
    \/ Reset
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

ProgressNext ==
    \/ DaemonUp
    \/ DaemonDown
    \/ Enqueue
    \/ StartSynth
    \/ SynthOk
    \/ SynthFail
    \/ InvalidTargetFail
    \/ CancelByExternalRequest
    \/ CancelByClientDisconnect
    \/ CancelByWriteFailure
    \/ CancelByShutdown
    \/ Reset

ProgressSpec ==
    /\ Init
    /\ [][Next]_vars
    /\ WF_vars(SynthOk
               \/ SynthFail
               \/ InvalidTargetFail
               \/ CancelByExternalRequest
               \/ CancelByClientDisconnect
               \/ CancelByWriteFailure
               \/ CancelByShutdown
               \/ DaemonDown)

EventuallyLeavesSynthesizing ==
    [](synthState = "Synthesizing" => <>(synthState # "Synthesizing"))

NormalNext ==
    \/ DaemonUp
    \/ Enqueue
    \/ StartSynth
    \/ SynthOk
    \/ Reset
    \/ Stutter

NormalSpec ==
    Init /\ [][NormalNext]_vars

NormalFlowNoFailure ==
    synthState \notin {"Failed", "Canceled"}

InvalidTargetNext ==
    \/ DaemonUp
    \/ Enqueue
    \/ InvalidTargetFail
    \/ Reset
    \/ Stutter

InvalidTargetSpec ==
    Init /\ [][InvalidTargetNext]_vars

=============================================================================
