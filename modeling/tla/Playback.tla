--------------------------------- MODULE Playback ---------------------------------
(***************************************************************************)
(* Playback lifecycle model separated from client/network concerns.        *)
(* Focus: player start, playing, stop, cancel, and failure handling.      *)
(***************************************************************************)

EXTENDS Naturals

VARIABLES audioReady, playbackState, cancelRequested, errorKind

vars == << audioReady, playbackState, cancelRequested, errorKind >>

TypeOK ==
    /\ audioReady \in BOOLEAN
    /\ playbackState \in {"Idle", "Launching", "Playing", "Stopped", "Failed"}
    /\ cancelRequested \in BOOLEAN
    /\ errorKind \in {"None", "LaunchFailed", "DeviceError", "Canceled"}

PlayingRequiresAudio ==
    playbackState = "Playing" => audioReady

CanceledImpliesStoppedOrFailed ==
    cancelRequested => playbackState \in {"Stopped", "Failed", "Launching", "Playing"}

Init ==
    /\ audioReady = FALSE
    /\ playbackState = "Idle"
    /\ cancelRequested = FALSE
    /\ errorKind = "None"

AudioArrived ==
    /\ playbackState = "Idle"
    /\ ~audioReady
    /\ audioReady' = TRUE
    /\ UNCHANGED << playbackState, cancelRequested, errorKind >>

StartPlayback ==
    /\ playbackState = "Idle"
    /\ audioReady
    /\ playbackState' = "Launching"
    /\ cancelRequested' = FALSE
    /\ errorKind' = "None"
    /\ UNCHANGED audioReady

LaunchOk ==
    /\ playbackState = "Launching"
    /\ ~cancelRequested
    /\ playbackState' = "Playing"
    /\ UNCHANGED << audioReady, cancelRequested, errorKind >>

LaunchFail ==
    /\ playbackState = "Launching"
    /\ playbackState' = "Failed"
    /\ errorKind' = "LaunchFailed"
    /\ UNCHANGED << audioReady, cancelRequested >>

Cancel ==
    /\ playbackState \in {"Launching", "Playing"}
    /\ cancelRequested' = TRUE
    /\ UNCHANGED << audioReady, playbackState, errorKind >>

StopByCancel ==
    /\ playbackState \in {"Launching", "Playing"}
    /\ cancelRequested
    /\ playbackState' = "Stopped"
    /\ errorKind' = "Canceled"
    /\ UNCHANGED << audioReady, cancelRequested >>

DeviceFailDuringPlay ==
    /\ playbackState = "Playing"
    /\ playbackState' = "Failed"
    /\ errorKind' = "DeviceError"
    /\ UNCHANGED << audioReady, cancelRequested >>

NaturalEnd ==
    /\ playbackState = "Playing"
    /\ ~cancelRequested
    /\ playbackState' = "Stopped"
    /\ errorKind' = "None"
    /\ UNCHANGED << audioReady, cancelRequested >>

Reset ==
    /\ playbackState \in {"Stopped", "Failed"}
    /\ playbackState' = "Idle"
    /\ audioReady' = FALSE
    /\ cancelRequested' = FALSE
    /\ errorKind' = "None"

Stutter ==
    UNCHANGED vars

Next ==
    \/ AudioArrived
    \/ StartPlayback
    \/ LaunchOk
    \/ LaunchFail
    \/ Cancel
    \/ StopByCancel
    \/ DeviceFailDuringPlay
    \/ NaturalEnd
    \/ Reset
    \/ Stutter

Spec ==
    Init /\ [][Next]_vars

=============================================================================
