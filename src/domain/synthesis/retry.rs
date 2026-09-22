//! Pure retry/backoff decision rules for the client-side synthesis loop.
//!
//! Mirrors the retry contract in `modeling/quint/SynthesisRetry.qnt`: the
//! initial attempt plus at most `max_retries` retries, and backoff only between
//! attempts. The classification of *which* errors are retryable stays outside
//! this module (it depends on the daemon error codes); callers pass an abstract
//! [`AttemptOutcome`].
//!
//! Cancellation priority is an orchestration concern and is not modelled here.

use std::time::Duration;

/// Outcome of a single synthesis attempt, from the retry policy's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttemptOutcome {
    /// The attempt produced a result.
    Succeeded,
    /// The attempt failed with a retryable error.
    RetryableFailure,
    /// The attempt failed with a non-retryable error.
    FatalFailure,
}

/// What to do after an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    /// Stop the loop (success, fatal failure, or retries exhausted).
    Finish,
    /// Wait for a backoff, then start another attempt.
    Backoff,
}

/// Retry policy for the client-side synthesis loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    max_retries: u32,
}

impl RetryPolicy {
    #[must_use]
    pub const fn new(max_retries: u32) -> Self {
        Self { max_retries }
    }

    #[must_use]
    pub const fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Total attempts allowed: the initial attempt plus `max_retries` retries.
    #[must_use]
    pub const fn max_attempts(&self) -> u32 {
        self.max_retries + 1
    }

    /// Decides what to do after an attempt.
    ///
    /// `attempts_started` is the number of attempts already started, including
    /// the one that just finished.
    #[must_use]
    pub const fn after_attempt(
        &self,
        attempts_started: u32,
        outcome: AttemptOutcome,
    ) -> RetryDecision {
        match outcome {
            AttemptOutcome::Succeeded | AttemptOutcome::FatalFailure => RetryDecision::Finish,
            AttemptOutcome::RetryableFailure => {
                if attempts_started < self.max_attempts() {
                    RetryDecision::Backoff
                } else {
                    RetryDecision::Finish
                }
            }
        }
    }

    /// Exponential backoff delay for the `retry_index`-th retry (0-based),
    /// doubling from `initial` and capped at `max`.
    #[must_use]
    pub fn backoff_delay(&self, retry_index: u32, initial: Duration, max: Duration) -> Duration {
        let mut delay = initial;
        for _ in 0..retry_index {
            delay = (delay * 2).min(max);
        }
        delay
    }
}

/// Phase of the client-side retry loop.
///
/// Mirrors the `Outcome` sum type in `modeling/quint/SynthesisRetry.qnt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryPhase {
    Running,
    Attempting,
    Backoff,
    Done,
    Failed,
    Canceled,
}

/// Pure state machine for the retry loop's phase and attempt/backoff counting.
///
/// `run_retry_loop` drives this for the real async loop; the model-based test
/// `mbt/tests/synthesis_retry.rs` drives the same type one spec step at a time,
/// so the counters and transitions are checked directly against
/// `modeling/quint/SynthesisRetry.qnt` (attempts and backoffs are counted at
/// start, and cancellation is terminal).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryTracker {
    policy: RetryPolicy,
    phase: RetryPhase,
    attempts_started: u32,
    backoffs_started: u32,
}

impl RetryTracker {
    #[must_use]
    pub const fn new(policy: RetryPolicy) -> Self {
        Self {
            policy,
            phase: RetryPhase::Running,
            attempts_started: 0,
            backoffs_started: 0,
        }
    }

    #[must_use]
    pub const fn phase(&self) -> RetryPhase {
        self.phase
    }

    #[must_use]
    pub const fn attempts_started(&self) -> u32 {
        self.attempts_started
    }

    #[must_use]
    pub const fn backoffs_started(&self) -> u32 {
        self.backoffs_started
    }

    #[must_use]
    pub fn can_begin_attempt(&self) -> bool {
        self.phase == RetryPhase::Running && self.attempts_started < self.policy.max_attempts()
    }

    /// Moves `Running` to `Attempting`, counting the attempt at start.
    ///
    /// Returns false when no attempt may start (not `Running`, or the attempt
    /// budget is exhausted).
    pub fn begin_attempt(&mut self) -> bool {
        if !self.can_begin_attempt() {
            return false;
        }
        self.phase = RetryPhase::Attempting;
        self.attempts_started += 1;
        true
    }

    /// Records the in-flight attempt's result, updates the phase, and returns
    /// the retry decision. A retryable failure that will be retried counts a
    /// backoff at start.
    #[must_use]
    pub fn record_attempt(&mut self, outcome: AttemptOutcome) -> RetryDecision {
        let decision = self.policy.after_attempt(self.attempts_started, outcome);
        match (outcome, decision) {
            (AttemptOutcome::Succeeded, _) => self.phase = RetryPhase::Done,
            (AttemptOutcome::FatalFailure, _) => self.phase = RetryPhase::Failed,
            (AttemptOutcome::RetryableFailure, RetryDecision::Backoff) => {
                self.backoffs_started += 1;
                self.phase = RetryPhase::Backoff;
            }
            (AttemptOutcome::RetryableFailure, RetryDecision::Finish) => {
                self.phase = RetryPhase::Failed;
            }
        }
        decision
    }

    /// Moves `Backoff` back to `Running`. Returns false in any other phase.
    pub fn end_backoff(&mut self) -> bool {
        if self.phase != RetryPhase::Backoff {
            return false;
        }
        self.phase = RetryPhase::Running;
        true
    }

    /// Cancels a non-terminal loop. Returns false if already terminal.
    pub fn cancel(&mut self) -> bool {
        if !matches!(
            self.phase,
            RetryPhase::Running | RetryPhase::Attempting | RetryPhase::Backoff
        ) {
            return false;
        }
        self.phase = RetryPhase::Canceled;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{AttemptOutcome, RetryDecision, RetryPhase, RetryPolicy, RetryTracker};
    use std::time::Duration;

    #[test]
    fn success_finishes_immediately() {
        let policy = RetryPolicy::new(2);
        assert_eq!(
            policy.after_attempt(1, AttemptOutcome::Succeeded),
            RetryDecision::Finish
        );
    }

    #[test]
    fn fatal_failure_finishes_immediately() {
        let policy = RetryPolicy::new(2);
        assert_eq!(
            policy.after_attempt(1, AttemptOutcome::FatalFailure),
            RetryDecision::Finish
        );
    }

    #[test]
    fn retryable_failure_backs_off_until_attempts_exhausted() {
        let policy = RetryPolicy::new(2);
        assert_eq!(policy.max_attempts(), 3);
        assert_eq!(
            policy.after_attempt(1, AttemptOutcome::RetryableFailure),
            RetryDecision::Backoff
        );
        assert_eq!(
            policy.after_attempt(2, AttemptOutcome::RetryableFailure),
            RetryDecision::Backoff
        );
        assert_eq!(
            policy.after_attempt(3, AttemptOutcome::RetryableFailure),
            RetryDecision::Finish
        );
    }

    #[test]
    fn tracker_counts_attempts_and_backoffs_at_start() {
        let mut tracker = RetryTracker::new(RetryPolicy::new(2));
        assert_eq!(tracker.phase(), RetryPhase::Running);
        assert!(tracker.begin_attempt());
        assert_eq!(tracker.attempts_started(), 1);
        assert_eq!(
            tracker.record_attempt(AttemptOutcome::RetryableFailure),
            RetryDecision::Backoff
        );
        assert_eq!(tracker.backoffs_started(), 1);
        assert_eq!(tracker.phase(), RetryPhase::Backoff);
        assert!(tracker.end_backoff());
        assert!(tracker.begin_attempt());
        assert_eq!(tracker.attempts_started(), 2);
        assert_eq!(
            tracker.record_attempt(AttemptOutcome::RetryableFailure),
            RetryDecision::Backoff
        );
        assert!(tracker.end_backoff());
        assert!(tracker.begin_attempt());
        assert_eq!(tracker.attempts_started(), 3);
        // Third retryable failure exhausts the budget; no backoff is counted.
        assert_eq!(
            tracker.record_attempt(AttemptOutcome::RetryableFailure),
            RetryDecision::Finish
        );
        assert_eq!(tracker.phase(), RetryPhase::Failed);
        assert_eq!(tracker.backoffs_started(), 2);
    }

    #[test]
    fn tracker_cancel_is_terminal() {
        let mut tracker = RetryTracker::new(RetryPolicy::new(2));
        assert!(tracker.cancel());
        assert_eq!(tracker.phase(), RetryPhase::Canceled);
        assert!(!tracker.cancel());
        assert!(!tracker.begin_attempt());
        assert!(!tracker.end_backoff());
    }

    #[test]
    fn backoff_delay_doubles_and_caps() {
        let policy = RetryPolicy::new(3);
        let initial = Duration::from_millis(100);
        let max = Duration::from_millis(400);
        assert_eq!(policy.backoff_delay(0, initial, max), initial);
        assert_eq!(
            policy.backoff_delay(1, initial, max),
            Duration::from_millis(200)
        );
        assert_eq!(policy.backoff_delay(2, initial, max), max);
        assert_eq!(policy.backoff_delay(5, initial, max), max);
    }
}
