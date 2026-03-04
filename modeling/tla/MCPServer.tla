-------------------------------- MODULE MCPServer --------------------------------
(***************************************************************************)
(* Client-side connection/retry with shared playback submodel.             *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_ATTEMPTS
ASSUME MAX_ATTEMPTS \in Nat

VARIABLES daemonState, clientState, attempt,
          serviceMode,
          playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind

vars == << daemonState, clientState, attempt,
           serviceMode,
           playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

P == INSTANCE Playback WITH
    audioReady <- playbackAudioReady,
    playbackState <- playbackState,
    cancelRequested <- playbackCancelRequested,
    errorKind <- playbackErrorKind

TypeOK ==
    /\ daemonState \in {"DaemonDown", "Ready"}
    /\ clientState \in {"Idle", "Connecting", "Connected"}
    /\ attempt \in 0..MAX_ATTEMPTS
    /\ serviceMode \in {"Normal", "Degraded"}
    /\ P!TypeOK

ConnectedImpliesDaemonReady ==
    clientState = "Connected" => daemonState = "Ready"

DegradedImpliesNotConnected ==
    serviceMode = "Degraded" => clientState # "Connected"

PlayingRequiresAudio ==
    P!PlayingRequiresAudio

Init ==
    /\ daemonState = "DaemonDown"
    /\ clientState = "Idle"
    /\ attempt = 0
    /\ serviceMode = "Normal"
    /\ P!Init

StartConnect ==
    /\ clientState = "Idle"
    /\ clientState' = "Connecting"
    /\ UNCHANGED << daemonState, attempt, serviceMode,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

ConnectOk ==
    /\ clientState = "Connecting"
    /\ daemonState = "Ready"
    /\ clientState' = "Connected"
    /\ serviceMode' = "Normal"
    /\ UNCHANGED << daemonState, attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

ConnectRetry ==
    /\ clientState = "Connecting"
    /\ daemonState = "DaemonDown"
    /\ attempt < MAX_ATTEMPTS
    /\ clientState' = "Idle"
    /\ attempt' = attempt + 1
    /\ UNCHANGED << daemonState, serviceMode,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

EnterDegraded ==
    /\ clientState = "Connecting"
    /\ daemonState = "DaemonDown"
    /\ attempt = MAX_ATTEMPTS
    /\ clientState' = "Idle"
    /\ serviceMode' = "Degraded"
    /\ UNCHANGED << daemonState, attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

LeaveDegraded ==
    /\ serviceMode = "Degraded"
    /\ clientState = "Idle"
    /\ clientState' = clientState
    /\ serviceMode' = "Normal"
    /\ attempt' = 0
    /\ UNCHANGED << daemonState,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

DaemonReady ==
    /\ daemonState = "DaemonDown"
    /\ daemonState' = "Ready"
    /\ UNCHANGED << clientState, attempt, serviceMode,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

DaemonDown ==
    /\ daemonState = "Ready"
    /\ daemonState' = "DaemonDown"
    /\ clientState' = IF clientState = "Connected" THEN "Idle" ELSE clientState
    /\ serviceMode' =
        IF clientState = "Connected" THEN "Degraded" ELSE serviceMode
    /\ UNCHANGED << attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

ReceiveAudio ==
    /\ clientState = "Connected"
    /\ P!AudioArrived
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackStart ==
    /\ clientState = "Connected"
    /\ P!StartPlayback
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackLaunchOk ==
    /\ P!LaunchOk
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackLaunchFail ==
    /\ P!LaunchFail
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackCancel ==
    /\ P!Cancel
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackStopByCancel ==
    /\ P!StopByCancel
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackDeviceFail ==
    /\ P!DeviceFailDuringPlay
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackNaturalEnd ==
    /\ P!NaturalEnd
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

PlaybackReset ==
    /\ P!Reset
    /\ UNCHANGED << daemonState, clientState, attempt, serviceMode >>

Next ==
    \/ StartConnect
    \/ ConnectOk
    \/ ConnectRetry
    \/ EnterDegraded
    \/ LeaveDegraded
    \/ DaemonReady
    \/ DaemonDown
    \/ ReceiveAudio
    \/ PlaybackStart
    \/ PlaybackLaunchOk
    \/ PlaybackLaunchFail
    \/ PlaybackCancel
    \/ PlaybackStopByCancel
    \/ PlaybackDeviceFail
    \/ PlaybackNaturalEnd
    \/ PlaybackReset

Spec ==
    Init /\ [][Next]_vars

=============================================================================
