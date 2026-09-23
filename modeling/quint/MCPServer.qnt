// Client-side connection/retry with the shared playback submodel.
// Flattened: all variables are assigned in every action because Quint does not
// implicitly frame unmentioned variables.
//
// - `MAX_ATTEMPTS` matches the production connect budget
//   `daemon::startup::MAX_CONNECT_ATTEMPTS` (10).
// - `transport::connect_with_retry` performs up to `MAX_ATTEMPTS` loop connects
//   with backoff and then one **final** connect without backoff; the model has
//   explicit `finalConnectOk` / `finalConnectFail` for that.
// - There is no persistent `Degraded` service mode: a failed connect episode
//   ends in the terminal `ConnectFailed` state (the request errors); a new
//   request resets to `ClientIdle`.
//
// Properties: `ConnectedImpliesDaemonReady`, `PlayingRequiresAudio`.

module MCPServer {
  pure val MAX_ATTEMPTS = 10

  type DaemonState = DaemonDown | DaemonReady
  type ClientState = ClientIdle | Connecting | Connected | ConnectFailed
  type PlaybackState = PlaybackIdle | Launching | Playing | Stopped | Failed
  type PlaybackError = NoPlaybackError | LaunchFailed | DeviceError | Canceled

  var daemonState: DaemonState
  var clientState: ClientState
  var attempt: int
  var playbackAudioReady: bool
  var playbackState: PlaybackState
  var playbackCancelRequested: bool
  var playbackErrorKind: PlaybackError

  action init = all {
    daemonState' = DaemonDown,
    clientState' = ClientIdle,
    attempt' = 0,
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
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action connectOk = all {
    clientState == Connecting,
    daemonState == DaemonReady,
    attempt < MAX_ATTEMPTS,
    clientState' = Connected,
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
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action finalConnectOk = all {
    clientState == Connecting,
    daemonState == DaemonReady,
    attempt == MAX_ATTEMPTS,
    clientState' = Connected,
    daemonState' = daemonState,
    attempt' = attempt,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action finalConnectFail = all {
    clientState == Connecting,
    daemonState == DaemonDown,
    attempt == MAX_ATTEMPTS,
    clientState' = ConnectFailed,
    daemonState' = daemonState,
    attempt' = attempt,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action resetConnectFailed = all {
    clientState == ConnectFailed,
    clientState' = ClientIdle,
    attempt' = 0,
    daemonState' = daemonState,
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
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action daemonDown = all {
    daemonState == DaemonReady,
    daemonState' = DaemonDown,
    clientState' = if (clientState == Connected) ClientIdle else clientState,
    // Losing a connection ends the connect episode; the next request starts a
    // fresh `connect_with_retry` budget.
    attempt' = if (clientState == Connected) 0 else attempt,
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
    playbackAudioReady' = playbackAudioReady,
  }

  action playbackLaunchOk = all {
    playbackState == Launching,
    not(playbackCancelRequested),
    playbackState' = Playing,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
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
    playbackAudioReady' = playbackAudioReady,
    playbackCancelRequested' = playbackCancelRequested,
  }

  action playbackCancel = all {
    playbackState == Launching or playbackState == Playing,
    playbackCancelRequested' = true,
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
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
  }

  action stutter = all {
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action step = any {
    startConnect,
    connectOk,
    connectRetry,
    finalConnectOk,
    finalConnectFail,
    resetConnectFailed,
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

  val playingRequiresAudio = playbackState == Playing implies playbackAudioReady

  // ---- Fixed scenarios (connect-budget MBT) -------------------------------
  //
  // Terminal connect states of `retry_with_final`, used by
  // `mbt/tests/mcp_connect.rs`. The driver runs the production retry loop with a
  // counting fake connector and maps the number of attempts made.

  action hold = all {
    daemonState' = daemonState,
    clientState' = clientState,
    attempt' = attempt,
    playbackAudioReady' = playbackAudioReady,
    playbackState' = playbackState,
    playbackCancelRequested' = playbackCancelRequested,
    playbackErrorKind' = playbackErrorKind,
  }

  action scenarioConnectedFirst = all {
    daemonState' = DaemonReady,
    clientState' = Connected,
    attempt' = 0,
    playbackAudioReady' = false,
    playbackState' = PlaybackIdle,
    playbackCancelRequested' = false,
    playbackErrorKind' = NoPlaybackError,
  }

  action scenarioConnectedAfterRetries = all {
    daemonState' = DaemonReady,
    clientState' = Connected,
    attempt' = 3,
    playbackAudioReady' = false,
    playbackState' = PlaybackIdle,
    playbackCancelRequested' = false,
    playbackErrorKind' = NoPlaybackError,
  }

  action scenarioConnectFailed = all {
    daemonState' = DaemonDown,
    clientState' = ConnectFailed,
    attempt' = MAX_ATTEMPTS,
    playbackAudioReady' = false,
    playbackState' = PlaybackIdle,
    playbackCancelRequested' = false,
    playbackErrorKind' = NoPlaybackError,
  }
}
