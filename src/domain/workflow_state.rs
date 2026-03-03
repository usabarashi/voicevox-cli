#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPhase {
    InitialConnect,
    ValidateModels,
    StartDaemon,
    ConnectRetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpStartupPhase {
    InitialStart,
    RecoverAlreadyRunning { pid: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynthesisPhase {
    Validate,
    EnsureResources,
    Connect,
    Synthesize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SayPhase {
    Validate,
    Synthesize,
    Emit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpTtsPhase {
    Attempt,
    Backoff,
    Finish,
}
