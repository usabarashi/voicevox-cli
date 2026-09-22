// Ported from `modeling/tla/Daemon.tla` (Phase 2), revised.
//
// Minimal daemon startup/recovery model focused on state-space control.
// Preserved properties: `SocketImpliesReady`, `BusyImpliesReady`,
// `RetryBounded`, `AlreadyRunningNotBusy`.
//
// `RecoveryPathExists` is intentionally **not** checked (retracted, see
// PORTING.md): it does not hold under weak fairness because `daemonFail` can be
// taken repeatedly from `Recovering` and exhaust `retryCount`. The retry bound
// mirrors the production connect budget `daemon::startup::MAX_CONNECT_ATTEMPTS`
// (10); the CLI/MCP readiness waits use 18/12 respectively and are not modelled
// here.

module Daemon {
  pure val MAX_RETRY = 10

  type DaemonState = DaemonDown | Starting | AlreadyRunning | Ready | Recovering
  type SocketState = SocketAbsent | SocketReady
  type ReqState = Idle | Busy

  var daemonState: DaemonState
  var socketState: SocketState
  var reqState: ReqState
  var retryCount: int

  action init = all {
    daemonState' = DaemonDown,
    socketState' = SocketAbsent,
    reqState' = Idle,
    retryCount' = 0,
  }

  action startDaemon = all {
    daemonState == DaemonDown,
    daemonState' = Starting,
    socketState' = socketState,
    reqState' = reqState,
    retryCount' = retryCount,
  }

  action daemonReady = all {
    daemonState == Starting,
    daemonState' = Ready,
    socketState' = SocketReady,
    reqState' = reqState,
    retryCount' = retryCount,
  }

  action alreadyRunningDetected = all {
    daemonState == Starting,
    daemonState' = AlreadyRunning,
    socketState' = socketState,
    reqState' = reqState,
    retryCount' = retryCount,
  }

  action alreadyRunningResponsive = all {
    daemonState == AlreadyRunning,
    daemonState' = Ready,
    socketState' = SocketReady,
    reqState' = reqState,
    retryCount' = retryCount,
  }

  // Recovery failures are budget-guarded: once the retry budget is exhausted,
  // `daemonFail` is disabled and the only way out of `Recovering` is `giveUp`.
  action alreadyRunningUnresponsive = all {
    daemonState == AlreadyRunning,
    retryCount < MAX_RETRY,
    daemonState' = Recovering,
    socketState' = SocketAbsent,
    reqState' = Idle,
    retryCount' = retryCount + 1,
  }

  action daemonFail = all {
    daemonState == Starting or daemonState == Recovering,
    retryCount < MAX_RETRY,
    daemonState' = Recovering,
    socketState' = SocketAbsent,
    reqState' = Idle,
    retryCount' = retryCount + 1,
  }

  action crashFromReady = all {
    daemonState == Ready,
    daemonState' = Recovering,
    socketState' = SocketAbsent,
    reqState' = Idle,
    retryCount' = if (retryCount < MAX_RETRY) retryCount + 1 else retryCount,
  }

  action recover = all {
    daemonState == Recovering,
    retryCount < MAX_RETRY,
    daemonState' = Starting,
    socketState' = socketState,
    reqState' = reqState,
    retryCount' = retryCount,
  }

  action giveUp = all {
    daemonState == Recovering,
    retryCount == MAX_RETRY,
    daemonState' = DaemonDown,
    socketState' = SocketAbsent,
    reqState' = Idle,
    retryCount' = retryCount,
  }

  action acceptReq = all {
    daemonState == Ready,
    socketState == SocketReady,
    reqState == Idle,
    reqState' = Busy,
    daemonState' = daemonState,
    socketState' = socketState,
    retryCount' = retryCount,
  }

  action finishReq = all {
    reqState == Busy,
    reqState' = Idle,
    daemonState' = daemonState,
    socketState' = socketState,
    retryCount' = retryCount,
  }

  action step = any {
    startDaemon,
    daemonReady,
    alreadyRunningDetected,
    alreadyRunningResponsive,
    alreadyRunningUnresponsive,
    daemonFail,
    crashFromReady,
    recover,
    giveUp,
    acceptReq,
    finishReq,
  }

  val typeOK = retryCount >= 0 and retryCount <= MAX_RETRY

  val socketImpliesReady = socketState == SocketReady implies daemonState == Ready

  val busyImpliesReady =
    reqState == Busy implies (daemonState == Ready and socketState == SocketReady)

  val retryBounded = retryCount <= MAX_RETRY

  val alreadyRunningNotBusy = daemonState == AlreadyRunning implies reqState == Idle

  // NOTE: `RecoveryPathExists` from the TLA+ model is intentionally NOT ported
  // as a checked property. No cfg scenario checked it, and it does not hold
  // under weak fairness: `daemonFail` can be taken repeatedly from `Recovering`,
  // exhausting `retryCount` and reaching `giveUp` without `recover` ever being
  // taken. See PORTING.md (retraction).
}
