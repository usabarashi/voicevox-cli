#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesisPhase {
    Validate,
    EnsureResources,
    Connect,
    Synthesize,
}
