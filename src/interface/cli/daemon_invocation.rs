#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStartMode {
    Foreground,
    Detached,
}

impl DaemonStartMode {
    #[must_use]
    pub fn from_flags(foreground: bool, detach: bool) -> Self {
        if detach && !foreground {
            Self::Detached
        } else {
            Self::Foreground
        }
    }

    #[must_use]
    pub const fn is_foreground(self) -> bool {
        matches!(self, Self::Foreground)
    }

    #[must_use]
    pub const fn should_detach(self) -> bool {
        matches!(self, Self::Detached)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonControlCommand {
    None,
    Stop,
    Status,
    Restart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonCliFlags {
    pub start_mode: DaemonStartMode,
    pub mode_flag_explicit: bool,
    pub start: bool,
    pub control: DaemonControlCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonInvocation {
    ShowUsage,
    Stop,
    Status,
    Restart,
    Start,
}

#[must_use]
pub const fn decide_daemon_invocation(flags: DaemonCliFlags) -> DaemonInvocation {
    match flags.control {
        DaemonControlCommand::Stop => DaemonInvocation::Stop,
        DaemonControlCommand::Status => DaemonInvocation::Status,
        DaemonControlCommand::Restart => DaemonInvocation::Restart,
        DaemonControlCommand::None if !flags.start && !flags.mode_flag_explicit => {
            DaemonInvocation::ShowUsage
        }
        DaemonControlCommand::None => DaemonInvocation::Start,
    }
}
