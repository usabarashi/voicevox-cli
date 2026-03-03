----------------------------------- MODULE System ----------------------------------
(***************************************************************************)
(* Top-level integrated model: FirstStartup + Client + Synthesis.          *)
(* Daemon availability is sourced from FirstStartup and synchronized to     *)
(* client/synthesis views in a single transition family.                   *)
(***************************************************************************)

EXTENDS Naturals

CONSTANTS MAX_RETRY, MAX_ATTEMPTS
ASSUME MAX_RETRY \in Nat
ASSUME MAX_ATTEMPTS \in Nat

VARIABLES fsRuntimeState, fsRuntimeRetry,
          fsDictState, fsDictRetry,
          fsSocketState, fsSocketRetry,
          fsModelState, fsModelRetry,
          fsDaemonState,
          clientDaemonState, clientState, clientAttempt,
          synthDaemonReady, synthState, synthRetryCount, synthErrorKind

vars == << fsRuntimeState, fsRuntimeRetry,
           fsDictState, fsDictRetry,
           fsSocketState, fsSocketRetry,
           fsModelState, fsModelRetry,
           fsDaemonState,
           clientDaemonState, clientState, clientAttempt,
           synthDaemonReady, synthState, synthRetryCount, synthErrorKind >>

FS == INSTANCE FirstStartup WITH
    MAX_RETRY <- MAX_RETRY,
    runtimeState <- fsRuntimeState,
    runtimeRetry <- fsRuntimeRetry,
    dictState <- fsDictState,
    dictRetry <- fsDictRetry,
    socketState <- fsSocketState,
    socketRetry <- fsSocketRetry,
    modelState <- fsModelState,
    modelRetry <- fsModelRetry,
    daemonState <- fsDaemonState

C == INSTANCE Client WITH
    MAX_ATTEMPTS <- MAX_ATTEMPTS,
    daemonState <- clientDaemonState,
    clientState <- clientState,
    attempt <- clientAttempt

Y == INSTANCE Synthesis WITH
    MAX_RETRY <- MAX_RETRY,
    daemonReady <- synthDaemonReady,
    synthState <- synthState,
    retryCount <- synthRetryCount,
    errorKind <- synthErrorKind

TypeOK ==
    /\ FS!TypeOK
    /\ C!TypeOK
    /\ Y!TypeOK

ViewsAligned ==
    /\ clientDaemonState =
        IF fsDaemonState = "Ready" THEN "Ready" ELSE "DaemonDown"
    /\ synthDaemonReady = (fsDaemonState = "Ready")

ClientConnectedImpliesDaemonReady ==
    clientState = "Connected" => fsDaemonState = "Ready"

SynthesisRunningImpliesDaemonReady ==
    synthState = "Synthesizing" => fsDaemonState = "Ready"

Init ==
    /\ FS!Init
    /\ clientDaemonState = "DaemonDown"
    /\ clientState = "Idle"
    /\ clientAttempt = 0
    /\ synthDaemonReady = FALSE
    /\ synthState = "Idle"
    /\ synthRetryCount = 0
    /\ synthErrorKind = "None"

SyncViewsAndDependentStates ==
    /\ clientDaemonState' =
        IF fsDaemonState' = "Ready" THEN "Ready" ELSE "DaemonDown"
    /\ synthDaemonReady' = (fsDaemonState' = "Ready")
    /\ clientState' =
        IF fsDaemonState' # "Ready" /\ clientState = "Connected"
        THEN "Idle"
        ELSE clientState
    /\ clientAttempt' = clientAttempt
    /\ synthState' =
        IF fsDaemonState' # "Ready" /\ synthState \in {"Queued", "Synthesizing"}
        THEN "Idle"
        ELSE synthState
    /\ synthRetryCount' = synthRetryCount
    /\ synthErrorKind' =
        IF fsDaemonState' # "Ready" /\ synthState \in {"Queued", "Synthesizing"}
        THEN "None"
        ELSE synthErrorKind

FirstStartupStep ==
    /\ FS!Next
    /\ SyncViewsAndDependentStates

ClientStep ==
    /\ \/ C!StartConnect
       \/ C!ConnectOk
       \/ C!ConnectRetry
       \/ C!ConnectFail
    /\ UNCHANGED << fsRuntimeState, fsRuntimeRetry,
                    fsDictState, fsDictRetry,
                    fsSocketState, fsSocketRetry,
                    fsModelState, fsModelRetry,
                    fsDaemonState,
                    synthDaemonReady, synthState, synthRetryCount, synthErrorKind >>

SynthesisStep ==
    /\ \/ Y!Enqueue
       \/ Y!StartSynth
       \/ Y!SynthOk
       \/ Y!SynthFail
       \/ Y!InvalidTargetFail
       \/ Y!Cancel
       \/ Y!Reset
    /\ UNCHANGED << fsRuntimeState, fsRuntimeRetry,
                    fsDictState, fsDictRetry,
                    fsSocketState, fsSocketRetry,
                    fsModelState, fsModelRetry,
                    fsDaemonState,
                    clientDaemonState, clientState, clientAttempt >>

Stutter ==
    UNCHANGED vars

Next ==
    \/ FirstStartupStep
    \/ ClientStep
    \/ SynthesisStep
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

=============================================================================
