---------------------------------- MODULE Say ----------------------------------
(***************************************************************************)
(* voicevox-say flow integrated with shared Daemon + Playback models.     *)
(* Validate -> Synthesize -> Emit is modeled while playback uses INSTANCE. *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_RETRY
ASSUME MAX_RETRY \in Nat

VARIABLES dDaemonState, dSocketState, dReqState, dRetryCount,
          pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
          textState, sayState, outputMode, errorKind

vars == << dDaemonState, dSocketState, dReqState, dRetryCount,
           pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
           textState, sayState, outputMode, errorKind >>

D == INSTANCE Daemon WITH
    MAX_RETRY <- MAX_RETRY,
    daemonState <- dDaemonState,
    socketState <- dSocketState,
    reqState <- dReqState,
    retryCount <- dRetryCount

P == INSTANCE Playback WITH
    audioReady <- pAudioReady,
    playbackState <- pPlaybackState,
    cancelRequested <- pCancelRequested,
    errorKind <- pPlaybackError

TypeOK ==
    /\ D!TypeOK
    /\ P!TypeOK
    /\ textState \in {"Unknown", "Valid", "Invalid"}
    /\ sayState \in {"Idle", "Validated", "Synthesizing", "Synthesized", "Emitting", "Done", "Failed"}
    /\ outputMode \in {"Play", "WriteFile"}
    /\ errorKind \in {"None", "Validation", "Synthesis", "Playback"}

SynthesizingImpliesBusyReq ==
    sayState = "Synthesizing" => /\ dReqState = "Busy"
                                 \/ dDaemonState # "Ready"

BusyReqOwnedBySay ==
    dReqState = "Busy" => sayState = "Synthesizing"

DoneHasNoError ==
    sayState = "Done" => errorKind = "None"

PlaybackFailureOnlyInPlayMode ==
    errorKind = "Playback" => outputMode = "Play"

PlayingRequiresAudio ==
    P!PlayingRequiresAudio

EmittingUsesPlayMode ==
    sayState = "Emitting" => outputMode = "Play"

Init ==
    /\ D!Init
    /\ P!Init
    /\ textState = "Unknown"
    /\ sayState = "Idle"
    /\ outputMode = "Play"
    /\ errorKind = "None"

DaemonInfraStep ==
    /\ \/ D!StartDaemon
       \/ /\ D!DaemonReady
          /\ ~(sayState = "Synthesizing" /\ dReqState = "Idle")
       \/ D!DaemonFail
       \/ D!CrashFromReady
       \/ D!Recover
       \/ D!GiveUp
    /\ UNCHANGED << pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, sayState, outputMode, errorKind >>

ChooseValidText ==
    /\ sayState = "Idle"
    /\ textState = "Unknown"
    /\ textState' = "Valid"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    sayState, outputMode, errorKind >>

ChooseInvalidText ==
    /\ sayState = "Idle"
    /\ textState = "Unknown"
    /\ textState' = "Invalid"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    sayState, outputMode, errorKind >>

ChoosePlayOutput ==
    /\ sayState = "Idle"
    /\ outputMode' = "Play"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, sayState, errorKind >>

ChooseFileOutput ==
    /\ sayState = "Idle"
    /\ outputMode' = "WriteFile"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, sayState, errorKind >>

ValidateOk ==
    /\ sayState = "Idle"
    /\ textState = "Valid"
    /\ sayState' = "Validated"
    /\ errorKind' = "None"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, outputMode >>

ValidateFail ==
    /\ sayState = "Idle"
    /\ textState = "Invalid"
    /\ sayState' = "Failed"
    /\ errorKind' = "Validation"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, outputMode >>

StartSynthesize ==
    /\ sayState = "Validated"
    /\ dDaemonState = "Ready"
    /\ dSocketState = "SocketReady"
    /\ dReqState = "Idle"
    /\ D!AcceptReq
    /\ sayState' = "Synthesizing"
    /\ errorKind' = "None"
    /\ UNCHANGED << pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, outputMode >>

SynthesizeSuccess ==
    /\ sayState = "Synthesizing"
    /\ dReqState = "Busy"
    /\ D!FinishReq
    /\ sayState' = "Synthesized"
    /\ errorKind' = "None"
    /\ UNCHANGED << pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, outputMode >>

SynthesizeFailNoDaemon ==
    /\ sayState = "Validated"
    /\ dDaemonState # "Ready"
    /\ sayState' = "Failed"
    /\ errorKind' = "Synthesis"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, outputMode >>

SynthesizeFailAfterCrash ==
    /\ sayState = "Synthesizing"
    /\ dReqState = "Idle"
    /\ sayState' = "Failed"
    /\ errorKind' = "Synthesis"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, outputMode >>

EmitToFile ==
    /\ sayState = "Synthesized"
    /\ outputMode = "WriteFile"
    /\ sayState' = "Done"
    /\ errorKind' = "None"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    textState, outputMode >>

EmitPlaybackBufferReady ==
    /\ sayState = "Synthesized"
    /\ outputMode = "Play"
    /\ P!AudioArrived
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, sayState, outputMode, errorKind >>

EmitPlaybackStart ==
    /\ sayState = "Synthesized"
    /\ outputMode = "Play"
    /\ P!StartPlayback
    /\ sayState' = "Emitting"
    /\ errorKind' = "None"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, outputMode >>

EmitPlaybackLaunchOk ==
    /\ sayState = "Emitting"
    /\ P!LaunchOk
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, sayState, outputMode, errorKind >>

EmitPlaybackNaturalEnd ==
    /\ sayState = "Emitting"
    /\ P!NaturalEnd
    /\ sayState' = "Done"
    /\ errorKind' = "None"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, outputMode >>

EmitPlaybackLaunchFail ==
    /\ sayState = "Emitting"
    /\ P!LaunchFail
    /\ sayState' = "Failed"
    /\ errorKind' = "Playback"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, outputMode >>

EmitPlaybackDeviceFail ==
    /\ sayState = "Emitting"
    /\ P!DeviceFailDuringPlay
    /\ sayState' = "Failed"
    /\ errorKind' = "Playback"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, outputMode >>

EmitPlaybackCancel ==
    /\ sayState = "Emitting"
    /\ P!Cancel
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, sayState, outputMode, errorKind >>

EmitPlaybackStopByCancel ==
    /\ sayState = "Emitting"
    /\ P!StopByCancel
    /\ sayState' = "Failed"
    /\ errorKind' = "Playback"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    textState, outputMode >>

ResetNoPlayback ==
    /\ sayState \in {"Done", "Failed"}
    /\ pPlaybackState = "Idle"
    /\ textState' = "Unknown"
    /\ sayState' = "Idle"
    /\ errorKind' = "None"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    pAudioReady, pPlaybackState, pCancelRequested, pPlaybackError,
                    outputMode >>

ResetWithPlayback ==
    /\ sayState \in {"Done", "Failed"}
    /\ pPlaybackState \in {"Stopped", "Failed"}
    /\ P!Reset
    /\ textState' = "Unknown"
    /\ sayState' = "Idle"
    /\ errorKind' = "None"
    /\ UNCHANGED << dDaemonState, dSocketState, dReqState, dRetryCount,
                    outputMode >>

Stutter ==
    UNCHANGED vars

Next ==
    \/ DaemonInfraStep
    \/ ChooseValidText
    \/ ChooseInvalidText
    \/ ChoosePlayOutput
    \/ ChooseFileOutput
    \/ ValidateOk
    \/ ValidateFail
    \/ StartSynthesize
    \/ SynthesizeSuccess
    \/ SynthesizeFailNoDaemon
    \/ SynthesizeFailAfterCrash
    \/ EmitToFile
    \/ EmitPlaybackBufferReady
    \/ EmitPlaybackStart
    \/ EmitPlaybackLaunchOk
    \/ EmitPlaybackNaturalEnd
    \/ EmitPlaybackLaunchFail
    \/ EmitPlaybackDeviceFail
    \/ EmitPlaybackCancel
    \/ EmitPlaybackStopByCancel
    \/ ResetNoPlayback
    \/ ResetWithPlayback
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

=============================================================================
