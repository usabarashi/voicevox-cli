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

#[cfg(test)]
mod tests {
    use super::{AttemptOutcome, RetryDecision, RetryPolicy};
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
