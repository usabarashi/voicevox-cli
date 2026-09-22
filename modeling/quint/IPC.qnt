// Ported from `modeling/tla/IPC.tla` (Phase 2), revised.
//
// IPC request/response model for the client transport
// (`src/infrastructure/daemon/client/transport.rs`).
//
// Production contract: `send_request_and_receive_response` performs **one**
// serialized request and one response read. There is no retry inside an attempt.
// Frame sizes are bounded by `MAX_DAEMON_REQUEST_FRAME_BYTES` (server, 256 KiB)
// and `MAX_DAEMON_RESPONSE_FRAME_BYTES` (client codec, 128 MiB); the response
// read has a 30 s timeout.
//
// Revisions vs the TLA+ port:
// - Removed `timeoutCount`/`MAX_TIMEOUTS`: it gated no action and had no
//   production counterpart.
// - Added the transport failure boundary: encode failure, write failure, EOF
//   (no response), oversized frame, and a protocol `Error` response.
//
// Preserved properties: `FailedImpliesError`, `DoneImpliesValidResponse`,
// and the temporal `EventuallyLeavesInFlight`.

module IPC {
  pure val MAX_REQUEST_FRAME_BYTES = 262144
  pure val MAX_RESPONSE_FRAME_BYTES = 134217728
  pure val RESPONSE_TIMEOUT_SECONDS = 30

  type ConnState = Disconnected | Connected
  type ReqState = Idle | InFlight | Done | Failed
  type RespState =
    | NoResponse
    | Valid
    | Corrupt
    | Mismatched
    | Timeout
    | Absent
    | TooLarge
    | ProtocolError
  type IpcError =
    | NoError
    | EncodeFailed
    | WriteFailed
    | CorruptFrame
    | ResponseMismatch
    | ResponseTimeout
    | NoResponseReceived
    | FrameTooLarge
    | ErrorResponse

  var connState: ConnState
  var reqState: ReqState
  var respState: RespState
  var ipcError: IpcError

  action init = all {
    connState' = Disconnected,
    reqState' = Idle,
    respState' = NoResponse,
    ipcError' = NoError,
  }

  action connect = all {
    connState == Disconnected,
    connState' = Connected,
    reqState' = reqState,
    respState' = respState,
    ipcError' = ipcError,
  }

  action disconnect = all {
    connState == Connected,
    connState' = Disconnected,
    reqState' = Idle,
    respState' = NoResponse,
    ipcError' = NoError,
  }

  // Encoding the request fails before anything is written.
  action encodeFailed = all {
    connState == Connected,
    reqState == Idle,
    reqState' = Failed,
    respState' = Absent,
    ipcError' = EncodeFailed,
    connState' = connState,
  }

  action sendRequest = all {
    connState == Connected,
    reqState == Idle,
    reqState' = InFlight,
    respState' = NoResponse,
    ipcError' = NoError,
    connState' = connState,
  }

  // Writing the frame fails after the request was considered in flight.
  action writeFailed = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Absent,
    ipcError' = WriteFailed,
    connState' = connState,
  }

  action receiveValidResponse = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Done,
    respState' = Valid,
    ipcError' = NoError,
    connState' = connState,
  }

  action corruptFrame = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Corrupt,
    ipcError' = CorruptFrame,
    connState' = connState,
  }

  action responseMismatch = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Mismatched,
    ipcError' = ResponseMismatch,
    connState' = connState,
  }

  action responseTimeout = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Timeout,
    ipcError' = ResponseTimeout,
    connState' = connState,
  }

  // The peer closed the stream before a response arrived.
  action noResponse = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = Absent,
    ipcError' = NoResponseReceived,
    connState' = connState,
  }

  // The frame exceeds the codec limit.
  action frameTooLarge = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = TooLarge,
    ipcError' = FrameTooLarge,
    connState' = connState,
  }

  // A syntactically valid `Error` response from the daemon.
  action protocolError = all {
    connState == Connected,
    reqState == InFlight,
    reqState' = Failed,
    respState' = ProtocolError,
    ipcError' = ErrorResponse,
    connState' = connState,
  }

  action resetRequest = all {
    reqState == Done or reqState == Failed,
    reqState' = Idle,
    respState' = NoResponse,
    ipcError' = NoError,
    connState' = connState,
  }

  action stutter = all {
    connState' = connState,
    reqState' = reqState,
    respState' = respState,
    ipcError' = ipcError,
  }

  action step = any {
    connect,
    disconnect,
    encodeFailed,
    sendRequest,
    writeFailed,
    receiveValidResponse,
    corruptFrame,
    responseMismatch,
    responseTimeout,
    noResponse,
    frameTooLarge,
    protocolError,
    resetRequest,
    stutter,
  }

  val failedImpliesError = reqState == Failed implies ipcError != NoError

  val doneImpliesValidResponse = reqState == Done implies respState == Valid

  val inFlightHasNoError = reqState == InFlight implies ipcError == NoError

  // Matches the old `ProgressSpec` fairness set, extended with the transport
  // failures that can resolve an in-flight request.
  action respond = any {
    writeFailed,
    receiveValidResponse,
    corruptFrame,
    responseMismatch,
    responseTimeout,
    noResponse,
    frameTooLarge,
    protocolError,
    disconnect,
  }

  val vars = (connState, reqState, respState, ipcError)

  temporal eventuallyLeavesInFlight =
    weakFair(respond, vars) implies
      always(reqState == InFlight implies eventually(reqState != InFlight))

  // ---- Fixed scenarios (daemon-free transport MBT) ------------------------
  //
  // Terminal states that the production client transport must reach for a fake
  // server's injected response, used by `mbt/tests/ipc_transport.rs`. A valid
  // response must yield `Done`; every injected fault must yield `Failed` with a
  // non-`NoError` error (the driver injects the response class and observes the
  // request state).

  action hold = all {
    connState' = connState,
    reqState' = reqState,
    respState' = respState,
    ipcError' = ipcError,
  }

  action scenarioValid = all {
    connState' = Connected,
    reqState' = Done,
    respState' = Valid,
    ipcError' = NoError,
  }

  action scenarioCorrupt = all {
    connState' = Connected,
    reqState' = Failed,
    respState' = Corrupt,
    ipcError' = CorruptFrame,
  }

  action scenarioMismatch = all {
    connState' = Connected,
    reqState' = Failed,
    respState' = Mismatched,
    ipcError' = ResponseMismatch,
  }

  action scenarioNoResponse = all {
    connState' = Connected,
    reqState' = Failed,
    respState' = Absent,
    ipcError' = NoResponseReceived,
  }

  action scenarioProtocolError = all {
    connState' = Connected,
    reqState' = Failed,
    respState' = ProtocolError,
    ipcError' = ErrorResponse,
  }
}
