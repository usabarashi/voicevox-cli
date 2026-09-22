// Ported from `modeling/tla/MCPServer.tla` (Phase 2).
//
// Client-side connection/retry with the shared playback submodel.
// Flattened (see PORTING.md): all variables are assigned in every action
// because Quint does not implicitly frame unmentioned variables.
// Preserved properties: `ConnectedImpliesDaemonReady`,
// `DegradedImpliesNotConnected`, `PlayingRequiresAudio`.

module MCPServer {
  pure val MAX_ATTEMPTS = 3

  type DaemonState = DaemonDown | DaemonReady
  type ClientState = ClientIdle | Connecting | Connected
  type ServiceMode = Normal | Degraded
  type PlaybackState = PlaybackIdle | Launching | Playing | Stopped | Failed
  type PlaybackError = NoPlaybackError | LaunchFailed | DeviceError | Canceled

  var daemonState: DaemonState
  var clientState: ClientState
  var attempt: int
  var serviceMode: ServiceMode
  var playbackAudioReady: bool
  var playbackState: PlaybackState
  var playbackCancelRequested: bool
  var playbackErrorKind: PlaybackError

  action init = all {
    daemonState' = DaemonDown,
    clientState' = ClientIdle,
    attempt' = 0,
    serviceMode' = Normal,
    playbackAudioReady' = false,
    playbackState' = PlaybackIdle,
    playbackCancelRequested' = false,
    playbackErrorKind' = NoPlaybackError,
  }

  action startConnect = all {
    clientState == ClientIdle,
    clientState' = Connecting,
    daemonState' = daemonState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action connectOk = all {
    clientState == Connecting,
    daemonState == DaemonReady,
    clientState' = Connected,
    serviceMode' = Normal,
    daemonState' = daemonState,
    attempt' = attempt,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action connectRetry = all {
    clientState == Connecting,
    daemonState == DaemonDown,
    attempt < MAX_ATTEMPTS,
    clientState' = ClientIdle,
    attempt' = attempt + 1,
    daemonState' = daemonState,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action enterDegraded = all {
    clientState == Connecting,
    daemonState == DaemonDown,
    attempt == MAX_ATTEMPTS,
    clientState' = ClientIdle,
    serviceMode' = Degraded,
    daemonState' = daemonState,
    attempt' = attempt,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action leaveDegraded = all {
    serviceMode == Degraded,
    clientState == ClientIdle,
    serviceMode' = Normal,
    attempt' = 0,
    daemonState' = daemonState,
    clientState' = clientState,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action daemonReady = all {
    daemonState == DaemonDown,
    daemonState' = DaemonReady,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action daemonDown = all {
    daemonState == DaemonReady,
    daemonState' = DaemonDown,
    clientState' = if (clientState == Connected) ClientIdle else clientState,
    serviceMode' = if (clientState == Connected) Degraded else serviceMode,
    attempt' = attempt,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action receiveAudio = all {
    clientState == Connected,
    playbackState == PlaybackIdle,
    not(playbackAudioReady),
    playbackAudioReady' = true,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action playbackStart = all {
    clientState == Connected,
    playbackState == PlaybackIdle,
    playbackAudioReady,
    playbackState' = Launching,
    playbackCancelRequested' = false,
    playbackErrorKind' = NoPlaybackError,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
  }

  action playbackLaunchOk = all {
    playbackState == Launching,
    not(playbackCancelRequested),
    playbackState' = Playing,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action playbackLaunchFail = all {
    playbackState == Launching,
    playbackState' = Failed,
    playbackErrorKind' = LaunchFailed,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackCancelRequested' = playbackCancelRequested,
  }

  action playbackCancel = all {
    playbackState == Launching or playbackState == Playing,
    playbackCancelRequested' = true,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackErrorKind' = playbackErrorKind,
  }

  action playbackStopByCancel = all {
    (playbackState == Launching or playbackState == Playing) and playbackCancelRequested,
    playbackState' = Stopped,
    playbackErrorKind' = Canceled,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackCancelRequested' = playbackCancelRequested,
  }

  action playbackDeviceFail = all {
    playbackState == Playing,
    playbackState' = Failed,
    playbackErrorKind' = DeviceError,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackCancelRequested' = playbackCancelRequested,
  }

  action playbackNaturalEnd = all {
    playbackState == Playing,
    not(playbackCancelRequested),
    playbackState' = Stopped,
    playbackErrorKind' = NoPlaybackError,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackCancelRequested' = playbackCancelRequested,
  }

  action playbackReset = all {
    playbackState == Stopped or playbackState == Failed,
    playbackState' = PlaybackIdle,
    playbackAudioReady' = false,
    playbackCancelRequested' = false,
    playbackErrorKind' = NoPlaybackError,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
  }

  action stutter = all {
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    serviceMode' = serviceMode,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action step = any {
    startConnect,
    connectOk,
    connectRetry,
    enterDegraded,
    leaveDegraded,
    daemonReady,
    daemonDown,
    receiveAudio,
    playbackStart,
    playbackLaunchOk,
    playbackLaunchFail,
    playbackCancel,
    playbackStopByCancel,
    playbackDeviceFail,
    playbackNaturalEnd,
    playbackReset,
    stutter,
  }

  val typeOK = attempt >= 0 and attempt <= MAX_ATTEMPTS

  val connectedImpliesDaemonReady = clientState == Connected implies daemonState == DaemonReady

  val degradedImpliesNotConnected = serviceMode == Degraded implies clientState != Connected

  val playingRequiresAudio = playbackState == Playing implies playbackAudioReady
}
