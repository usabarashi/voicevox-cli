// Ported from `modeling/tla/Socket.tla` (Phase 2), revised.
//
// Minimal socket lifecycle for the daemon IPC endpoint. Re-binding after a
// permission denial is only possible through the budget-guarded `retryBind`.
//
// The historical `readyIsBounded` invariant was equivalent to `typeOK`; it is
// replaced by the temporal property `bindingTerminates` (a bind attempt does
// not get stuck in `Binding`).

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
    state == SocketAbsent,
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

  action advance = any {
    bindOk,
    bindPermissionDenied,
  }

  val vars = (state, retryCount)

  // A bind attempt always leaves `Binding` under weak fairness.
  temporal bindingTerminates =
    weakFair(advance, vars) implies
      always(state == Binding implies eventually(state != Binding))
}
