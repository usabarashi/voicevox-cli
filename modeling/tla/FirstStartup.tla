------------------------------- MODULE FirstStartup -------------------------------
(***************************************************************************)
(* Integrated first-startup model.                                         *)
(* Composes Runtime/Dictionary/Socket modules and checks download gates.   *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES runtimeState, runtimeRetry,
          dictState, dictRetry,
          socketState, socketRetry,
          modelState, modelRetry,
          daemonState

vars == << runtimeState, runtimeRetry,
           dictState, dictRetry,
           socketState, socketRetry,
           modelState, modelRetry,
           daemonState >>

R == INSTANCE Runtime WITH
    MAX_RETRY <- MAX_RETRY,
    runtimeState <- runtimeState,
    retryCount <- runtimeRetry

D == INSTANCE Dictionary WITH
    MAX_RETRY <- MAX_RETRY,
    dictState <- dictState,
    retryCount <- dictRetry

S == INSTANCE Socket WITH
    MAX_RETRY <- MAX_RETRY,
    socketState <- socketState,
    retryCount <- socketRetry

ResourceReady ==
    /\ runtimeState = "Ready"
    /\ dictState = "Ready"
    /\ modelState = "Ready"

TypeOK ==
    /\ R!TypeOK
    /\ D!TypeOK
    /\ S!TypeOK
    /\ modelState \in {"Missing", "Loading", "Ready", "Failed"}
    /\ modelRetry \in 0..MAX_RETRY
    /\ daemonState \in {"Down", "Starting", "Ready", "Failed"}

DaemonReadyRequiresDownloads ==
    daemonState = "Ready" => ResourceReady

DaemonStartRequiresDownloads ==
    daemonState = "Starting" => ResourceReady

DaemonReadyRequiresSocket ==
    daemonState = "Ready" => socketState = "SocketReady"

Init ==
    /\ R!Init
    /\ D!Init
    /\ S!Init
    /\ modelState = "Missing"
    /\ modelRetry = 0
    /\ daemonState = "Down"

RuntimeStep ==
    /\ \/ R!BeginLoad
       \/ R!LoadOk
       \/ R!LoadFail
       \/ R!RetryLoad
       \/ R!GiveUp
    /\ UNCHANGED << dictState, dictRetry,
                    socketState, socketRetry,
                    modelState, modelRetry,
                    daemonState >>

DictionaryStep ==
    /\ \/ D!BeginLoad
       \/ D!LoadOk
       \/ D!LoadFail
       \/ D!RetryLoad
       \/ D!GiveUp
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    socketState, socketRetry,
                    modelState, modelRetry,
                    daemonState >>

SocketStep ==
    /\ daemonState = "Starting"
    /\ \/ S!StartBind
       \/ S!BindOk
       \/ S!BindPermissionDenied
       \/ S!RetryBind
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    modelState, modelRetry,
                    daemonState >>

StartModelDownload ==
    /\ modelState \in {"Missing", "Failed"}
    /\ modelState' = "Loading"
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    socketState, socketRetry,
                    modelRetry, daemonState >>

ModelDownloadOk ==
    /\ modelState = "Loading"
    /\ modelState' = "Ready"
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    socketState, socketRetry,
                    modelRetry, daemonState >>

ModelDownloadFail ==
    /\ modelState = "Loading"
    /\ modelState' = "Failed"
    /\ modelRetry' = IF modelRetry < MAX_RETRY THEN modelRetry + 1 ELSE modelRetry
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    socketState, socketRetry,
                    daemonState >>

StartDaemon ==
    /\ daemonState = "Down"
    /\ ResourceReady
    /\ daemonState' = "Starting"
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    socketState, socketRetry,
                    modelState, modelRetry >>

DaemonBootOk ==
    /\ daemonState = "Starting"
    /\ socketState = "SocketReady"
    /\ daemonState' = "Ready"
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    socketState, socketRetry,
                    modelState, modelRetry >>

DaemonBootFail ==
    /\ daemonState = "Starting"
    /\ daemonState' = "Failed"
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    socketState, socketRetry,
                    modelState, modelRetry >>

ResetDaemon ==
    /\ daemonState = "Failed"
    /\ daemonState' = "Down"
    /\ UNCHANGED << runtimeState, runtimeRetry,
                    dictState, dictRetry,
                    socketState, socketRetry,
                    modelState, modelRetry >>

Stutter ==
    UNCHANGED vars

Next ==
    \/ RuntimeStep
    \/ DictionaryStep
    \/ SocketStep
    \/ StartModelDownload
    \/ ModelDownloadOk
    \/ ModelDownloadFail
    \/ StartDaemon
    \/ DaemonBootOk
    \/ DaemonBootFail
    \/ ResetDaemon
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

=============================================================================
