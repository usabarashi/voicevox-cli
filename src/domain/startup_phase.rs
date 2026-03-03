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
