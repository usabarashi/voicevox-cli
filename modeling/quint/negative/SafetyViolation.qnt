// Negative control: the retry loop omits the attempt bound, so the
// `attemptsBounded` safety property must be violated. Used by verify.sh to
// prove that the TLC backend actually detects safety violations.

module SafetyViolation {
  pure val MAX_RETRIES = 2
  pure val MAX_ATTEMPTS = MAX_RETRIES + 1

  var attempts: int

  action init = all {
    attempts' = 0,
  }

  // No upper bound on attempts.
  action step = all {
    attempts' = attempts + 1,
  }

  val attemptsBounded = attempts <= MAX_ATTEMPTS
}
