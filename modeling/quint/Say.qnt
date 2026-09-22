// Ported from `modeling/tla/Say.tla` (Phase 2).
//
// voicevox-say flow integrated with the shared Daemon + Playback models.
// Flattened (see PORTING.md); every action assigns every variable.
// Preserved properties: `SynthesizingImpliesBusyReq`, `BusyReqOwnedBySay`,
// `DoneHasNoError`, `PlaybackFailureOnlyInPlayMode`, `PlayingRequiresAudio`,
// `EmittingUsesPlayMode`.
//
// `MAX_RETRY` uses the `Say.daemon` cfg value (2). The `Say.standard` scenario
// (MAX_RETRY=0) differs only in the retry bound, which none of the preserved
// invariants depend on.

module Say {
  pure val MAX_RETRY = 2

  type DaemonState = DaemonDown | Starting | AlreadyRunning | Ready | Recovering
  type SocketState = SocketAbsent | SocketReady
  type ReqState = ReqIdle | Busy
  type PlaybackState = PlaybackIdle | Launching | Playing | Stopped | PlaybackFailed
  type PlaybackError = NoPlaybackError | LaunchFailed | DeviceError | Canceled
  type TextState = Unknown | Valid | Invalid
  type SayState = SayIdle | Validated | Synthesizing | Synthesized | Emitting | SayDone | SayFailed
  type OutputMode = Play | WriteFile
  type SayError = NoError | Validation | Synthesis | Playback

  var dDaemonState: DaemonState
  var dSocketState: SocketState
  var dReqState: ReqState
  var dRetryCount: int
  var pAudioReady: bool
  var pPlaybackState: PlaybackState
  var pCancelRequested: bool
  var pPlaybackError: PlaybackError
  var textState: TextState
  var sayState: SayState
  var outputMode: OutputMode
  var errorKind: SayError

  action init = all {
    dDaemonState' = DaemonDown, dSocketState' = SocketAbsent, dReqState' = ReqIdle, dRetryCount' = 0,
    pAudioReady' = false, pPlaybackState' = PlaybackIdle, pCancelRequested' = false, pPlaybackError' = NoPlaybackError,
    textState' = Unknown, sayState' = SayIdle, outputMode' = Play, errorKind' = NoError,
  }

  // ---- Daemon infrastructure ---------------------------------------------

  action startDaemon = all {
    dDaemonState == DaemonDown,
    dDaemonState' = Starting,
    dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action daemonReady = all {
    dDaemonState == Starting,
    // Guard from `Say.tla` DaemonInfraStep: do not complete a daemon-ready
    // transition that would leave a synthesizing say with an idle request.
    not(sayState == Synthesizing and dReqState == ReqIdle),
    dDaemonState' = Ready, dSocketState' = SocketReady,
    dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action daemonFail = all {
    dDaemonState == Starting or dDaemonState == Recovering,
    dDaemonState' = Recovering, dSocketState' = SocketAbsent, dReqState' = ReqIdle,
    dRetryCount' = if (dRetryCount < MAX_RETRY) dRetryCount + 1 else dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action crashFromReady = all {
    dDaemonState == Ready,
    dDaemonState' = Recovering, dSocketState' = SocketAbsent, dReqState' = ReqIdle,
    dRetryCount' = if (dRetryCount < MAX_RETRY) dRetryCount + 1 else dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action recover = all {
    dDaemonState == Recovering, dRetryCount < MAX_RETRY,
    dDaemonState' = Starting,
    dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action giveUp = all {
    dDaemonState == Recovering, dRetryCount == MAX_RETRY,
    dDaemonState' = DaemonDown, dSocketState' = SocketAbsent, dReqState' = ReqIdle,
    dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  // ---- Text / output selection -------------------------------------------

  action chooseValidText = all {
    sayState == SayIdle, textState == Unknown,
    textState' = Valid,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action chooseInvalidText = all {
    sayState == SayIdle, textState == Unknown,
    textState' = Invalid,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action choosePlayOutput = all {
    sayState == SayIdle,
    outputMode' = Play,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, errorKind' = errorKind,
  }

  action chooseFileOutput = all {
    sayState == SayIdle,
    outputMode' = WriteFile,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, errorKind' = errorKind,
  }

  action validateOk = all {
    sayState == SayIdle, textState == Valid,
    sayState' = Validated, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, outputMode' = outputMode,
  }

  action validateFail = all {
    sayState == SayIdle, textState == Invalid,
    sayState' = SayFailed, errorKind' = Validation,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, outputMode' = outputMode,
  }

  // ---- Synthesis ----------------------------------------------------------

  action startSynthesize = all {
    sayState == Validated, dDaemonState == Ready, dSocketState == SocketReady, dReqState == ReqIdle,
    dReqState' = Busy,
    sayState' = Synthesizing, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, outputMode' = outputMode,
  }

  action synthesizeSuccess = all {
    sayState == Synthesizing, dReqState == Busy,
    dReqState' = ReqIdle,
    sayState' = Synthesized, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, outputMode' = outputMode,
  }

  action synthesizeFailNoDaemon = all {
    sayState == Validated, dDaemonState != Ready,
    sayState' = SayFailed, errorKind' = Synthesis,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, outputMode' = outputMode,
  }

  action synthesizeFailAfterCrash = all {
    sayState == Synthesizing, dReqState == ReqIdle,
    sayState' = SayFailed, errorKind' = Synthesis,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, outputMode' = outputMode,
  }

  // ---- Emit ---------------------------------------------------------------

  action emitToFile = all {
    sayState == Synthesized, outputMode == WriteFile,
    sayState' = SayDone, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, outputMode' = outputMode,
  }

  action emitPlaybackBufferReady = all {
    sayState == Synthesized, outputMode == Play, pPlaybackState == PlaybackIdle, not(pAudioReady),
    pAudioReady' = true,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action emitPlaybackStart = all {
    sayState == Synthesized, outputMode == Play, pPlaybackState == PlaybackIdle, pAudioReady,
    pPlaybackState' = Launching, pCancelRequested' = false, pPlaybackError' = NoPlaybackError,
    sayState' = Emitting, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, textState' = textState, outputMode' = outputMode,
  }

  action emitPlaybackLaunchOk = all {
    sayState == Emitting, pPlaybackState == Launching, not(pCancelRequested),
    pPlaybackState' = Playing,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action emitPlaybackNaturalEnd = all {
    sayState == Emitting, pPlaybackState == Playing, not(pCancelRequested),
    pPlaybackState' = Stopped, pPlaybackError' = NoPlaybackError,
    sayState' = SayDone, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pCancelRequested' = pCancelRequested, textState' = textState, outputMode' = outputMode,
  }

  action emitPlaybackLaunchFail = all {
    sayState == Emitting, pPlaybackState == Launching,
    pPlaybackState' = PlaybackFailed, pPlaybackError' = LaunchFailed,
    sayState' = SayFailed, errorKind' = Playback,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pCancelRequested' = pCancelRequested, textState' = textState, outputMode' = outputMode,
  }

  action emitPlaybackDeviceFail = all {
    sayState == Emitting, pPlaybackState == Playing,
    pPlaybackState' = PlaybackFailed, pPlaybackError' = DeviceError,
    sayState' = SayFailed, errorKind' = Playback,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pCancelRequested' = pCancelRequested, textState' = textState, outputMode' = outputMode,
  }

  action emitPlaybackCancel = all {
    sayState == Emitting, pPlaybackState == Launching or pPlaybackState == Playing,
    pCancelRequested' = true,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action emitPlaybackStopByCancel = all {
    sayState == Emitting,
    (pPlaybackState == Launching or pPlaybackState == Playing) and pCancelRequested,
    pPlaybackState' = Stopped, pPlaybackError' = Canceled,
    sayState' = SayFailed, errorKind' = Playback,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pCancelRequested' = pCancelRequested, textState' = textState, outputMode' = outputMode,
  }

  // ---- Reset --------------------------------------------------------------

  action resetNoPlayback = all {
    (sayState == SayDone or sayState == SayFailed), pPlaybackState == PlaybackIdle,
    textState' = Unknown, sayState' = SayIdle, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    outputMode' = outputMode,
  }

  action resetWithPlayback = all {
    (sayState == SayDone or sayState == SayFailed),
    (pPlaybackState == Stopped or pPlaybackState == PlaybackFailed),
    pPlaybackState' = PlaybackIdle, pAudioReady' = false, pCancelRequested' = false, pPlaybackError' = NoPlaybackError,
    textState' = Unknown, sayState' = SayIdle, errorKind' = NoError,
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    outputMode' = outputMode,
  }

  action stutter = all {
    dDaemonState' = dDaemonState, dSocketState' = dSocketState, dReqState' = dReqState, dRetryCount' = dRetryCount,
    pAudioReady' = pAudioReady, pPlaybackState' = pPlaybackState, pCancelRequested' = pCancelRequested, pPlaybackError' = pPlaybackError,
    textState' = textState, sayState' = sayState, outputMode' = outputMode, errorKind' = errorKind,
  }

  action step = any {
    startDaemon,
    daemonReady,
    daemonFail,
    crashFromReady,
    recover,
    giveUp,
    chooseValidText,
    chooseInvalidText,
    choosePlayOutput,
    chooseFileOutput,
    validateOk,
    validateFail,
    startSynthesize,
    synthesizeSuccess,
    synthesizeFailNoDaemon,
    synthesizeFailAfterCrash,
    emitToFile,
    emitPlaybackBufferReady,
    emitPlaybackStart,
    emitPlaybackLaunchOk,
    emitPlaybackNaturalEnd,
    emitPlaybackLaunchFail,
    emitPlaybackDeviceFail,
    emitPlaybackCancel,
    emitPlaybackStopByCancel,
    resetNoPlayback,
    resetWithPlayback,
    stutter,
  }

  val typeOK = dRetryCount >= 0 and dRetryCount <= MAX_RETRY

  val synthesizingImpliesBusyReq =
    sayState == Synthesizing implies (dReqState == Busy or dDaemonState != Ready)

  val busyReqOwnedBySay = dReqState == Busy implies sayState == Synthesizing

  val doneHasNoError = sayState == SayDone implies errorKind == NoError

  val playbackFailureOnlyInPlayMode = errorKind == Playback implies outputMode == Play

  val playingRequiresAudio = pPlaybackState == Playing implies pAudioReady

  val emittingUsesPlayMode = sayState == Emitting implies outputMode == Play
}
