#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupPhase {
    InitialConnect,
    ValidateModels,
    StartDaemon,
    ConnectRetry,
}
