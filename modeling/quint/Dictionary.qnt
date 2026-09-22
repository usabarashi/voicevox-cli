// Ported from `modeling/tla/Dictionary.tla` (Phase 2), revised.
//
// Minimal dictionary lifecycle (e.g. OpenJTalk dictionary). A failed load may be
// retried only through the budget-guarded `retryLoad`.
//
// The historical `readyIsStable` invariant was equivalent to `typeOK`; it is
// replaced by the temporal property `loadedStaysReady` (the dictionary is not
// unloaded once ready).

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
    state == Missing,
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

  action stutter = all {
    state' = state,
    retryCount' = retryCount,
  }

  action step = any {
    beginLoad,
    loadOk,
    loadFail,
    retryLoad,
    stutter,
  }

  val typeOK = retryCount >= 0 and retryCount <= MAX_RETRY

  // Once the dictionary is ready it is never unloaded.
  temporal loadedStaysReady =
    always(state == Ready implies always(state == Ready))
}
