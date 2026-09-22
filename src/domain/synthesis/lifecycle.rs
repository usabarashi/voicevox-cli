//! Single-attempt synthesis lifecycle.
//!
//! Mirrors the model in `modeling/quint/SynthesisRetry.qnt` at the same
//! abstraction level: one attempt progresses `Idle -> Queued -> Synthesizing`
//! and then terminates as `Done`, `Failed`, or `Canceled`.
//!
//! Retry/backoff is a separate concern; see [`super::retry`].

/// Lifecycle of a single synthesis attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesisLifecycleState {
    Idle,
    Queued,
    Synthesizing,
    Done,
    Failed,
    Canceled,
}

impl SynthesisLifecycleState {
    /// Moves `Idle` to `Queued`; other states are unchanged.
    #[must_use]
    pub const fn queue(self) -> Self {
        match self {
            Self::Idle => Self::Queued,
            _ => self,
        }
    }

    /// Moves `Queued` to `Synthesizing`; other states are unchanged.
    #[must_use]
    pub const fn start(self) -> Self {
        match self {
            Self::Queued => Self::Synthesizing,
            _ => self,
        }
    }

    /// Moves `Synthesizing` to `Done`; other states are unchanged.
    #[must_use]
    pub const fn succeed(self) -> Self {
        match self {
            Self::Synthesizing => Self::Done,
            _ => self,
        }
    }

    /// Moves any non-terminal state to `Failed`; terminal states are unchanged.
    #[must_use]
    pub const fn fail(self) -> Self {
        match self {
            Self::Idle | Self::Queued | Self::Synthesizing => Self::Failed,
            _ => self,
        }
    }

    /// Moves a pending attempt to `Canceled`; terminal states are unchanged.
    #[must_use]
    pub const fn cancel(self) -> Self {
        match self {
            Self::Queued | Self::Synthesizing => Self::Canceled,
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SynthesisLifecycleState;

    #[test]
    fn happy_path_reaches_done() {
        let state = SynthesisLifecycleState::Idle
            .queue()
            .start()
            .succeed();
        assert_eq!(state, SynthesisLifecycleState::Done);
    }

    #[test]
    fn fail_from_any_non_terminal_state() {
        assert_eq!(
            SynthesisLifecycleState::Idle.fail(),
            SynthesisLifecycleState::Failed
        );
        assert_eq!(
            SynthesisLifecycleState::Queued.fail(),
            SynthesisLifecycleState::Failed
        );
        assert_eq!(
            SynthesisLifecycleState::Synthesizing.fail(),
            SynthesisLifecycleState::Failed
        );
    }

    #[test]
    fn cancel_only_from_pending_states() {
        assert_eq!(
            SynthesisLifecycleState::Queued.cancel(),
            SynthesisLifecycleState::Canceled
        );
        assert_eq!(
            SynthesisLifecycleState::Synthesizing.cancel(),
            SynthesisLifecycleState::Canceled
        );
        // Cancel does not resurrect terminal states.
        assert_eq!(
            SynthesisLifecycleState::Done.cancel(),
            SynthesisLifecycleState::Done
        );
    }

    #[test]
    fn terminal_states_are_stable() {
        for terminal in [
            SynthesisLifecycleState::Done,
            SynthesisLifecycleState::Failed,
            SynthesisLifecycleState::Canceled,
        ] {
            assert_eq!(terminal.queue(), terminal);
            assert_eq!(terminal.start(), terminal);
            assert_eq!(terminal.succeed(), terminal);
            assert_eq!(terminal.fail(), terminal);
        }
    }
}
