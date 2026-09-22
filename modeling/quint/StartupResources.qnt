// Ported from `modeling/tla/StartupResources.tla` (Phase 2).
//
// Startup resources and daemon bootstrap, composing the runtime/dictionary/
// socket/model lifecycles with daemon readiness.
//
// The TLA+ module uses `INSTANCE` composition with a shared `MAX_RETRY`. The
// TLC verification backend cannot assign constants through instances, so this
// port is flattened with a concrete `pure val MAX_RETRY`. The value mirrors the
// production resource installer `download_missing_resources` (`max_retries = 3`
// in `src/infrastructure/download/install.rs`).
// Preserved properties: `DaemonReadyRequiresDownloads`,
// `DaemonStartRequiresDownloads`, `DaemonReadyRequiresSocket`.

module StartupResources {
  pure val MAX_RETRY = 3

  type LoadState = Missing | Loading | Ready | Failed
  type SocketPhase = SocketAbsent | Binding | SocketReady | PermissionDenied
  type DaemonPhase = Down | DaemonStarting | DaemonReady | DaemonFailed

  var runtimeState: LoadState
  var runtimeRetry: int
  var dictState: LoadState
  var dictRetry: int
  var socketState: SocketPhase
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
    modelState' = Missing,
    modelRetry' = 0,
    daemonState' = Down,
  }

  // ---- ONNX Runtime ------------------------------------------------------

  action beginLoadRuntime = all {
    // First load only; re-entry after failure goes through `retryLoadRuntime`,
    // which enforces the retry budget.
    runtimeState == Missing,
    runtimeState' = Loading,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
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
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  // ---- Dictionary --------------------------------------------------------

  action beginLoadDict = all {
    // First load only; re-entry after failure goes through `retryLoadDict`.
    dictState == Missing,
    dictState' = Loading,
    dictRetry' = dictRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    socketState' = socketState,
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
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  // ---- Socket (only while the daemon is starting) ------------------------

  action startBind = all {
    daemonState == DaemonStarting,
    // A bind attempt is one-shot (production `UnixListener::bind`); permission
    // denial is terminal for the run.
    socketState == SocketAbsent,
    socketState' = Binding,
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
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    modelState' = modelState,
    modelRetry' = modelRetry,
    daemonState' = daemonState,
  }

  // The socket file is removed when the daemon is not running (recovered from
  // the retired `Socket.qnt`). Guarding on a non-ready daemon keeps
  // `daemonReadyRequiresSocket` true.
  action socketDrop = all {
    daemonState == Down or daemonState == DaemonFailed,
    socketState == SocketReady,
    socketState' = SocketAbsent,
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
    // First download only; re-entry after failure is not modelled as a retry
    // path here (production retries the whole installer, modelled by MAX_RETRY).
    modelState == Missing,
    modelState' = Loading,
    modelRetry' = modelRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
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
    daemonState' = daemonState,
  }

  action retryModelDownload = all {
    modelState == Failed,
    modelRetry < MAX_RETRY,
    modelState' = Loading,
    modelRetry' = modelRetry,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
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
    modelState' = modelState,
    modelRetry' = modelRetry,
  }

  // A running daemon stops and removes its socket. Leaving `DaemonReady` keeps
  // `daemonReadyRequiresSocket` true.
  action daemonStop = all {
    daemonState == DaemonReady,
    daemonState' = Down,
    socketState' = SocketAbsent,
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
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
    // A failed boot drops an in-progress bind (the socket is removed on
    // shutdown), so `Binding` never stays stuck. A terminal `PermissionDenied`
    // is left in place.
    socketState' = if (socketState == Binding) SocketAbsent else socketState,
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
    modelState' = modelState,
    modelRetry' = modelRetry,
  }

  action stutter = all {
    runtimeState' = runtimeState,
    runtimeRetry' = runtimeRetry,
    dictState' = dictState,
    dictRetry' = dictRetry,
    socketState' = socketState,
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
    socketDrop,
    startModelDownload,
    modelDownloadOk,
    modelDownloadFail,
    retryModelDownload,
    startDaemon,
    daemonBootOk,
    daemonStop,
    daemonBootFail,
    resetDaemon,
    stutter,
  }

  val typeOK =
    runtimeRetry >= 0 and runtimeRetry <= MAX_RETRY and
    dictRetry >= 0 and dictRetry <= MAX_RETRY and
    modelRetry >= 0 and modelRetry <= MAX_RETRY

  val daemonReadyRequiresDownloads =
    daemonState == DaemonReady implies resourceReady

  val daemonStartRequiresDownloads =
    daemonState == DaemonStarting implies resourceReady

  val daemonReadyRequiresSocket =
    daemonState == DaemonReady implies socketState == SocketReady

  // ---- Socket bind liveness (ported from the retired `Socket.qnt`) --------

  action bindAdvance = any {
    startBind,
    bindOk,
    bindPermissionDenied,
    daemonBootFail,
  }

  val bindVars = (socketState, daemonState)

  // A bind attempt always leaves `Binding` (success, permission denial, or a
  // daemon boot failure that drops the socket).
  temporal bindingTerminates =
    weakFair(bindAdvance, bindVars) implies
      always(socketState == Binding implies eventually(socketState != Binding))

  // A permission denial is terminal for that daemon run.
  temporal permissionDeniedIsTerminal =
    always(socketState == PermissionDenied implies always(socketState == PermissionDenied))
}
