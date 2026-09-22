// Ported from `modeling/tla/IPC.tla` (Phase 2).
//
// IPC request/response model with protocol-level failure branches
// (frame corruption, response mismatch, timeout).
// Preserved properties: `FailedImpliesError`, `DoneImpliesValidResponse`,
// and the temporal `EventuallyLeavesInFlight` under the old `ProgressSpec`
// fairness assumption.

module IPC {
  pure val MAX_TIMEOUTS = 3

  type ConnState = Disconnected | Connected
  type ReqState = Idle | InFlight | Done | Failed
  type RespState = NoResponse | Valid | Corrupt | Mismatched | Timeout
  type IpcError = NoError | CorruptFrame | ResponseMismatch | ResponseTimeout

  var connState: ConnState
  var reqState: ReqState
  var respState: RespState
  var ipcError: IpcError
  var timeoutCount: int

  action init = all {
    connState' = Disconnected,
    reqState' = Idle,
    respState' = NoResponse,
    ipcError' = NoError,
    timeoutCount' = 0,
  }

  action connect = all {
    connState == Disconnected,
    connState' = Connected,
    reqState' = reqState,
    respState' = respState,
    ipcError' = ipcError,
    timeoutCount' = timeoutCount,
  }

  action disconnect = all {
    connState == Connected,
    connState' = Disconnected,
    reqState' = Idle,
    respState' = NoResponse,
    ipcError' = NoError,
    timeoutCount' = timeoutCount,
  }

  action sendRequest = all {
    connState == Connected,
    reqState == Idle,
    reqState' = InFlight,
    respState' = NoResponse,
    ipcError' = NoError,
    connState' = connState,
    timeoutCount' = timeoutCount,
  }

  action receiveValidResponse = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Done,
    respState' = Valid,
    ipcError' = NoError,
    connState' = connState,
    timeoutCount' = timeoutCount,
  }

  action corruptFrame = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Corrupt,
    ipcError' = CorruptFrame,
    connState' = connState,
    timeoutCount' = timeoutCount,
  }

  action responseMismatch = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Mismatched,
    ipcError' = ResponseMismatch,
    connState' = connState,
    timeoutCount' = timeoutCount,
  }

  action responseTimeout = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Timeout,
    ipcError' = ResponseTimeout,
    connState' = connState,
    timeoutCount' = if (timeoutCount < MAX_TIMEOUTS) timeoutCount + 1 else timeoutCount,
  }

  action resetRequest = all {
    reqState == Done or reqState == Failed,
    reqState' = Idle,
    respState' = NoResponse,
    ipcError' = NoError,
    connState' = connState,
    timeoutCount' = timeoutCount,
  }

  action stutter = all {
    connState' = connState,
    reqState' = reqState,
    respState' = respState,
    ipcError' = ipcError,
    timeoutCount' = timeoutCount,
  }

  action step = any {
    connect,
    disconnect,
    sendRequest,
    receiveValidResponse,
    corruptFrame,
    responseMismatch,
    responseTimeout,
    resetRequest,
    stutter,
  }

  val typeOK = timeoutCount >= 0 and timeoutCount <= MAX_TIMEOUTS

  val failedImpliesError = reqState == Failed implies ipcError != NoError

  val doneImpliesValidResponse = reqState == Done implies respState == Valid

  // Matches the old `ProgressSpec` fairness set.
  action respond = any {
    receiveValidResponse,
    corruptFrame,
    responseMismatch,
    responseTimeout,
    disconnect,
  }

  val vars = (connState, reqState, respState, ipcError, timeoutCount)

  temporal eventuallyLeavesInFlight =
    weakFair(respond, vars) implies
      always(reqState == InFlight implies eventually(reqState != InFlight))
}
