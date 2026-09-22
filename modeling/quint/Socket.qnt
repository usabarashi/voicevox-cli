// Ported from `modeling/tla/Socket.tla` (Phase 2).
//
// Minimal socket lifecycle for the daemon IPC endpoint.
// Preserved properties: type bounds and `ReadyIsBounded`.

module Socket {
  pure val MAX_RETRY = 3

  type SocketState = SocketAbsent | Binding | SocketReady | PermissionDenied

  var state: SocketState
  var retryCount: int

  action init = all {
    state' = SocketAbsent,
    retryCount' = 0,
  }

  action startBind = all {
    state == SocketAbsent or state == PermissionDenied,
    state' = Binding,
    retryCount' = retryCount,
  }

  action bindOk = all {
    state == Binding,
    state' = SocketReady,
    retryCount' = retryCount,
  }

  action bindPermissionDenied = all {
    state == Binding,
    state' = PermissionDenied,
    retryCount' = if (retryCount < MAX_RETRY) retryCount + 1 else retryCount,
  }

  action socketDrop = all {
    state == SocketReady,
    state' = SocketAbsent,
    retryCount' = retryCount,
  }

  action retryBind = all {
    state == PermissionDenied,
    retryCount < MAX_RETRY,
    state' = Binding,
    retryCount' = retryCount,
  }

  action stutter = all {
    state' = state,
    retryCount' = retryCount,
  }

  action step = any {
    startBind,
    bindOk,
    bindPermissionDenied,
    socketDrop,
    retryBind,
    stutter,
  }

  val typeOK = retryCount >= 0 and retryCount <= MAX_RETRY
  val readyIsBounded = state == SocketReady implies retryCount <= MAX_RETRY
}
