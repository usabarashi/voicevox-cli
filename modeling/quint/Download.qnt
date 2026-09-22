// Download/install lifecycle model.
//
// Mirrors `download_missing_resources` in
// `src/infrastructure/download/install.rs`: the installer prepares the target
// directory and locates the downloader, then runs the external downloader,
// cleaning incomplete downloads before a retry, for at most `MAX_ATTEMPTS`
// total invocations, and finally cleans up and fails.
//
// This is the relocated home of the download lifecycle that `VoicevoxModel.tla`
// used to carry (see CONTRACT.md, contract change 5).

module Download {
  // Total downloader invocations (`max_retries = 3` in production).
  pure val MAX_ATTEMPTS = 3

  type DState = Idle | Downloading | Cleaning | Done | Failed
  type FailureKind = NoFailure | PreparationFailed | Exhausted

  var state: DState
  // Number of downloader invocations started.
  var attempts: int
  var failure: FailureKind

  action init = all {
    state' = Idle,
    attempts' = 0,
    failure' = NoFailure,
  }

  // `create_dir_all` or `find_downloader_binary` can fail before any download.
  action prepareFail = all {
    state == Idle,
    state' = Failed,
    attempts' = attempts,
    failure' = PreparationFailed,
  }

  action start = all {
    state == Idle,
    state' = Downloading,
    attempts' = attempts + 1,
    failure' = NoFailure,
  }

  action downloadOk = all {
    state == Downloading,
    state' = Done,
    attempts' = attempts,
    failure' = NoFailure,
  }

  // A failed invocation either cleans up before retrying, or gives up when the
  // budget is exhausted.
  action downloadFail = all {
    state == Downloading,
    attempts' = attempts,
    failure' = if (attempts >= MAX_ATTEMPTS) Exhausted else NoFailure,
    state' = if (attempts >= MAX_ATTEMPTS) Failed else Cleaning,
  }

  action cleanup = all {
    state == Cleaning,
    state' = Downloading,
    attempts' = attempts + 1,
    failure' = NoFailure,
  }

  action stutter = all {
    state' = state,
    attempts' = attempts,
    failure' = failure,
  }

  action step = any {
    prepareFail,
    start,
    downloadOk,
    downloadFail,
    cleanup,
    stutter,
  }

  val attemptsBounded = attempts >= 0 and attempts <= MAX_ATTEMPTS

  val failedHasReason = state == Failed implies failure != NoFailure

  val exhaustedImpliesAttempts = failure == Exhausted implies attempts == MAX_ATTEMPTS

  val preparationFailureMeansNoAttempts =
    failure == PreparationFailed implies attempts == 0

  action advance = any {
    prepareFail,
    start,
    downloadOk,
    downloadFail,
    cleanup,
  }

  val vars = (state, attempts, failure)

  temporal terminates =
    weakFair(advance, vars) implies eventually(state == Done or state == Failed)
}
