-------------------------------- MODULE MCPServer --------------------------------
(***************************************************************************)
(* Client-side connection/retry with shared playback submodel.             *)
(***************************************************************************)

EXTENDS Naturals

CONSTANT MAX_ATTEMPTS
ASSUME MAX_ATTEMPTS \in Nat

VARIABLES daemonState, clientState, attempt,
          playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind

vars == << daemonState, clientState, attempt,
           playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

P == INSTANCE Playback WITH
    audioReady <- playbackAudioReady,
    playbackState <- playbackState,
    cancelRequested <- playbackCancelRequested,
    errorKind <- playbackErrorKind

TypeOK ==
    /\ daemonState \in {"DaemonDown", "Ready"}
    /\ clientState \in {"Idle", "Connecting", "Connected", "Failed"}
    /\ attempt \in 0..MAX_ATTEMPTS
    /\ P!TypeOK

ConnectedImpliesDaemonReady ==
    clientState = "Connected" => daemonState = "Ready"

PlayingRequiresAudio ==
    P!PlayingRequiresAudio

Init ==
    /\ daemonState = "DaemonDown"
    /\ clientState = "Idle"
    /\ attempt = 0
    /\ P!Init

StartConnect ==
    /\ clientState = "Idle"
    /\ clientState' = "Connecting"
    /\ UNCHANGED << daemonState, attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

ConnectOk ==
    /\ clientState = "Connecting"
    /\ daemonState = "Ready"
    /\ clientState' = "Connected"
    /\ UNCHANGED << daemonState, attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

ConnectRetry ==
    /\ clientState = "Connecting"
    /\ daemonState = "DaemonDown"
    /\ attempt < MAX_ATTEMPTS
    /\ clientState' = "Idle"
    /\ attempt' = attempt + 1
    /\ UNCHANGED << daemonState,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

ConnectFail ==
    /\ clientState = "Connecting"
    /\ daemonState = "DaemonDown"
    /\ attempt = MAX_ATTEMPTS
    /\ clientState' = "Failed"
    /\ UNCHANGED << daemonState, attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

DaemonReady ==
    /\ daemonState = "DaemonDown"
    /\ daemonState' = "Ready"
    /\ UNCHANGED << clientState, attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

DaemonDown ==
    /\ daemonState = "Ready"
    /\ daemonState' = "DaemonDown"
    /\ clientState' = IF clientState = "Connected" THEN "Idle" ELSE clientState
    /\ UNCHANGED << attempt,
                    playbackAudioReady, playbackState, playbackCancelRequested, playbackErrorKind >>

ReceiveAudio ==
    /\ clientState = "Connected"
    /\ P!AudioArrived
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackStart ==
    /\ clientState = "Connected"
    /\ P!StartPlayback
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackLaunchOk ==
    /\ P!LaunchOk
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackLaunchFail ==
    /\ P!LaunchFail
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackCancel ==
    /\ P!Cancel
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackStopByCancel ==
    /\ P!StopByCancel
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackDeviceFail ==
    /\ P!DeviceFailDuringPlay
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackNaturalEnd ==
    /\ P!NaturalEnd
    /\ UNCHANGED << daemonState, clientState, attempt >>

PlaybackReset ==
    /\ P!Reset
    /\ UNCHANGED << daemonState, clientState, attempt >>

Next ==
    \/ StartConnect
    \/ ConnectOk
    \/ ConnectRetry
    \/ ConnectFail
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
