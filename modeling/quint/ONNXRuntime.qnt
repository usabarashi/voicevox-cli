// Ported from `modeling/tla/ONNXRuntime.tla` (Phase 2), revised.
//
// Minimal ONNX Runtime lifecycle (loader/setup state). A failed load may be
// retried only through the budget-guarded `retryLoad`, so the retry count is an
// operational bound (production retries the installer at most `MAX_RETRY`
// times; see `src/infrastructure/download/install.rs`).
//
// The historical `readyHasNoPendingRetry` invariant was equivalent to
// `typeOK`; it is replaced by the liveness property `loadTerminates`.

module ONNXRuntime {
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

  action advance = any {
    beginLoad,
    loadOk,
    loadFail,
    retryLoad,
  }

  val vars = (state, retryCount)

  // The load loop terminates: under weak fairness it reaches `Ready`, or gives
  // up with an exhausted retry budget.
  temporal loadTerminates =
    weakFair(advance, vars) implies
      eventually(state == Ready or (state == Failed and retryCount == MAX_RETRY))
}
