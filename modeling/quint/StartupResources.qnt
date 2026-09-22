// Ported from `modeling/tla/StartupResources.tla` (Phase 2).
//
// Startup resources and daemon bootstrap, composing the runtime/dictionary/
// socket/model lifecycles with daemon readiness.
//
// The TLA+ module uses `INSTANCE` composition with a shared `MAX_RETRY`. The
// TLC verification backend cannot assign constants through instances, so this
// port is flattened with a concrete `pure val MAX_RETRY` (the cfg value 2).
// Preserved properties: `DaemonReadyRequiresDownloads`,
// `DaemonStartRequiresDownloads`, `DaemonReadyRequiresSocket`.

module StartupResources {
  pure val MAX_RETRY = 2

  type LoadState = Missing | Loading | Ready | Failed
  type SocketPhase = SocketAbsent | Binding | SocketReady | PermissionDenied
  type DaemonPhase = Down | DaemonStarting | DaemonReady | DaemonFailed

  var runtimeState: LoadState
  var runtimeRetry: int
  var dictState: LoadState
  var dictRetry: int
  var socketState: SocketPhase
  var socketRetry: int
  var modelState: LoadState
  var modelRetry: int
  var daemonState: DaemonPhase

  val resourceReady =
    runtimeState == Ready and dictState == Ready and modelState == Ready

  action init = all {
    runtimeState' = Missing,
    runtimeRetry' = 0,
    dictState' = Missing,
    dictRetry' = 0,
    socketState' = SocketAbsent,
    socketRetry' = 0,
    modelState' = Missing,
    modelRetry' = 0,
    daemonState' = Down,
  }

  // ---- ONNX Runtime ------------------------------------------------------

  action beginLoadRuntime = all {
    runtimeState == Missing or runtimeState == Failed,
    runtimeState' = Loading,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action loadOkRuntime = all {
    runtimeState == Loading,
    runtimeState' = Ready,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action loadFailRuntime = all {
    runtimeState == Loading,
    runtimeState' = Failed,
    runtimeRetry' = if (runtimeRetry < MAX_RETRY) runtimeRetry + 1 else runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action retryLoadRuntime = all {
    runtimeState == Failed,
    runtimeRetry < MAX_RETRY,
    runtimeState' = Loading,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action giveUpRuntime = all {
    runtimeState == Failed,
    runtimeRetry == MAX_RETRY,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  // ---- Dictionary --------------------------------------------------------

  action beginLoadDict = all {
    dictState == Missing or dictState == Failed,
    dictState' = Loading,
    dictRetry' = dictRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action loadOkDict = all {
    dictState == Loading,
    dictState' = Ready,
    dictRetry' = dictRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action loadFailDict = all {
    dictState == Loading,
    dictState' = Failed,
    dictRetry' = if (dictRetry < MAX_RETRY) dictRetry + 1 else dictRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action retryLoadDict = all {
    dictState == Failed,
    dictRetry < MAX_RETRY,
    dictState' = Loading,
    dictRetry' = dictRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action giveUpDict = all {
    dictState == Failed,
    dictRetry == MAX_RETRY,
    dictState' = dictState,
    dictRetry' = dictRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  // ---- Socket (only while the daemon is starting) ------------------------

  action startBind = all {
    daemonState == DaemonStarting,
    socketState == SocketAbsent or socketState == PermissionDenied,
    socketState' = Binding,
    socketRetry' = socketRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action bindOk = all {
    daemonState == DaemonStarting,
    socketState == Binding,
    socketState' = SocketReady,
    socketRetry' = socketRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action bindPermissionDenied = all {
    daemonState == DaemonStarting,
    socketState == Binding,
    socketState' = PermissionDenied,
    socketRetry' = if (socketRetry < MAX_RETRY) socketRetry + 1 else socketRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action retryBind = all {
    daemonState == DaemonStarting,
    socketState == PermissionDenied,
    socketRetry < MAX_RETRY,
    socketState' = Binding,
    socketRetry' = socketRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  // ---- Model download ----------------------------------------------------

  action startModelDownload = all {
    modelState == Missing or modelState == Failed,
    modelState' = Loading,
    modelRetry' = modelRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    daemonState' = daemonState,
  }

  action modelDownloadOk = all {
    modelState == Loading,
    modelState' = Ready,
    modelRetry' = modelRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    daemonState' = daemonState,
  }

  action modelDownloadFail = all {
    modelState == Loading,
    modelState' = Failed,
    modelRetry' = if (modelRetry < MAX_RETRY) modelRetry + 1 else modelRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    daemonState' = daemonState,
  }

  // ---- Daemon ------------------------------------------------------------

  action startDaemon = all {
    daemonState == Down,
    resourceReady,
    daemonState' = DaemonStarting,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
  }

  action daemonBootOk = all {
    daemonState == DaemonStarting,
    socketState == SocketReady,
    daemonState' = DaemonReady,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
  }

  action daemonBootFail = all {
    daemonState == DaemonStarting,
    daemonState' = DaemonFailed,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
  }

  action resetDaemon = all {
    daemonState == DaemonFailed,
    daemonState' = Down,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
  }

  action stutter = all {
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
    socketRetry' = socketRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  action step = any {
    beginLoadRuntime,
    loadOkRuntime,
    loadFailRuntime,
    retryLoadRuntime,
    giveUpRuntime,
    beginLoadDict,
    loadOkDict,
    loadFailDict,
    retryLoadDict,
    giveUpDict,
    startBind,
    bindOk,
    bindPermissionDenied,
    retryBind,
    startModelDownload,
    modelDownloadOk,
    modelDownloadFail,
    startDaemon,
    daemonBootOk,
    daemonBootFail,
    resetDaemon,
    stutter,
  }

  val typeOK =
    runtimeRetry >= 0 and runtimeRetry <= MAX_RETRY and
    dictRetry >= 0 and dictRetry <= MAX_RETRY and
    socketRetry >= 0 and socketRetry <= MAX_RETRY and
    modelRetry >= 0 and modelRetry <= MAX_RETRY

  val daemonReadyRequiresDownloads =
    daemonState == DaemonReady implies resourceReady

  val daemonStartRequiresDownloads =
    daemonState == DaemonStarting implies resourceReady

  val daemonReadyRequiresSocket =
    daemonState == DaemonReady implies socketState == SocketReady
}
