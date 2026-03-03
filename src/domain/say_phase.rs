#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SayPhase {
    Validate,
    Synthesize,
    Emit,
}
