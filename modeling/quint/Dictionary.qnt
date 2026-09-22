// Ported from `modeling/tla/Dictionary.tla` (Phase 2).
//
// Minimal dictionary lifecycle (e.g. OpenJTalk dictionary).
// Preserved properties: type bounds and `ReadyIsStable`.

module Dictionary {
  pure val MAX_RETRY = 3

  type LoadState = Missing | Loading | Ready | Failed

  var state: LoadState
  var retryCount: int

  action init = all {
    state' = Missing,
    retryCount' = 0,
  }

  action beginLoad = all {
    state == Missing or state == Failed,
    state' = Loading,
    retryCount' = retryCount,
  }

  action loadOk = all {
    state == Loading,
    state' = Ready,
    retryCount' = retryCount,
  }

  action loadFail = all {
    state == Loading,
    state' = Failed,
    retryCount' = if (retryCount < MAX_RETRY) retryCount + 1 else retryCount,
  }

  action retryLoad = all {
    state == Failed,
    retryCount < MAX_RETRY,
    state' = Loading,
    retryCount' = retryCount,
  }

  action giveUp = all {
    state == Failed,
    retryCount == MAX_RETRY,
    state' = state,
    retryCount' = retryCount,
  }

  action stutter = all {
    state' = state,
    retryCount' = retryCount,
  }

  action step = any {
    beginLoad,
    loadOk,
    loadFail,
    retryLoad,
    giveUp,
    stutter,
  }

  val typeOK = retryCount >= 0 and retryCount <= MAX_RETRY
  val readyIsStable = state == Ready implies retryCount <= MAX_RETRY
}
