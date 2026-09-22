// Ported from `modeling/tla/Playback.tla` (Phase 2).
//
// Playback lifecycle separated from client/network concerns.
// Preserved properties: `PlayingRequiresAudio`, `CanceledImpliesStoppedOrFailed`.
// (`TypeOK` is subsumed by Quint's type system; see PORTING.md.)

module Playback {
  type PlaybackState = Idle | Launching | Playing | Stopped | Failed
  type PlaybackError = None | LaunchFailed | DeviceError | Canceled

  var audioReady: bool
  var playbackState: PlaybackState
  var cancelRequested: bool
  var errorKind: PlaybackError

  action init = all {
    audioReady' = false,
    playbackState' = Idle,
    cancelRequested' = false,
    errorKind' = None,
  }

  action audioArrived = all {
    playbackState == Idle,
    not(audioReady),
    audioReady' = true,
    playbackState' = playbackState,
    cancelRequested' = cancelRequested,
    errorKind' = errorKind,
  }

  action startPlayback = all {
    playbackState == Idle,
    audioReady,
    playbackState' = Launching,
    cancelRequested' = false,
    errorKind' = None,
    audioReady' = audioReady,
  }

  action launchOk = all {
    playbackState == Launching,
    not(cancelRequested),
    playbackState' = Playing,
    audioReady' = audioReady,
    cancelRequested' = cancelRequested,
    errorKind' = errorKind,
  }

  action launchFail = all {
    playbackState == Launching,
    playbackState' = Failed,
    errorKind' = LaunchFailed,
    audioReady' = audioReady,
    cancelRequested' = cancelRequested,
  }

  action cancel = all {
    playbackState == Launching or playbackState == Playing,
    cancelRequested' = true,
    audioReady' = audioReady,
    playbackState' = playbackState,
    errorKind' = errorKind,
  }

  action stopByCancel = all {
    playbackState == Launching or playbackState == Playing,
    cancelRequested,
    playbackState' = Stopped,
    errorKind' = Canceled,
    audioReady' = audioReady,
    cancelRequested' = cancelRequested,
  }

  action deviceFailDuringPlay = all {
    playbackState == Playing,
    playbackState' = Failed,
    errorKind' = DeviceError,
    audioReady' = audioReady,
    cancelRequested' = cancelRequested,
  }

  action naturalEnd = all {
    playbackState == Playing,
    not(cancelRequested),
    playbackState' = Stopped,
    errorKind' = None,
    audioReady' = audioReady,
    cancelRequested' = cancelRequested,
  }

  action reset = all {
    playbackState == Stopped or playbackState == Failed,
    playbackState' = Idle,
    audioReady' = false,
    cancelRequested' = false,
    errorKind' = None,
  }

  action stutter = all {
    audioReady' = audioReady,
    playbackState' = playbackState,
    cancelRequested' = cancelRequested,
    errorKind' = errorKind,
  }

  action step = any {
    audioArrived,
    startPlayback,
    launchOk,
    launchFail,
    cancel,
    stopByCancel,
    deviceFailDuringPlay,
    naturalEnd,
    reset,
    stutter,
  }

  val playingRequiresAudio = playbackState == Playing implies audioReady

  // The *Canceled error* implies the lifecycle has reached a terminal
  // stopped/failed state. (The previous formula used the pending
  // `cancelRequested` flag and admitted `Launching`/`Playing`, which made the
  // property vacuous for the production stop/kill behavior.)
  val canceledImpliesStoppedOrFailed =
    errorKind == Canceled implies
      (playbackState == Stopped or playbackState == Failed)

  // ---- Fixed scenarios (emit/play dispatch MBT) ---------------------------
  //
  // Terminal states of one `emit_and_play` call, used by
  // `mbt/tests/playback.rs`. The driver runs the production dispatch with a
  // scripted fake backend and maps the observed outcome.

  action hold = all {
    audioReady' = audioReady,
    playbackState' = playbackState,
    cancelRequested' = cancelRequested,
    errorKind' = errorKind,
  }

  action scenarioNaturalEnd = all {
    audioReady' = true,
    playbackState' = Stopped,
    cancelRequested' = false,
    errorKind' = None,
  }

  action scenarioCanceled = all {
    audioReady' = true,
    playbackState' = Stopped,
    cancelRequested' = true,
    errorKind' = Canceled,
  }

  action scenarioLaunchFailed = all {
    audioReady' = true,
    playbackState' = Failed,
    cancelRequested' = false,
    errorKind' = LaunchFailed,
  }
}
