use std::path::Path;

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

#[must_use]
pub fn daemon_usage_lines(version: &str) -> Vec<String> {
    vec![
        format!("VOICEVOX Daemon v{version}"),
        "\nDaemon Operations:".to_string(),
        "  --start     Start the daemon (default)".to_string(),
        "  --stop      Stop the running daemon".to_string(),
        "  --status    Check daemon status".to_string(),
        "  --restart   Restart the daemon".to_string(),
        "\nExecution Modes:".to_string(),
        "  --foreground Run in foreground (for development)".to_string(),
        "  --detach     Run as background process".to_string(),
        "\nUse --help for all options".to_string(),
    ]
}

#[must_use]
pub fn daemon_start_banner_lines(version: &str, socket_path: &Path) -> Vec<String> {
    vec![
        format!("VOICEVOX Daemon v{version}"),
        "Starting user daemon...".to_string(),
        format!("Socket: {} (user-specific)", socket_path.display()),
        "Models: Load and unload per request (no caching)".to_string(),
    ]
}

#[must_use]
pub fn daemon_socket_line(socket_path: &Path) -> String {
    format!("Socket: {}", socket_path.display())
}

#[must_use]
pub fn daemon_not_running_lines(socket_path: &Path) -> [String; 2] {
    [
        "Daemon is not running".to_string(),
        format!("   Socket: {}", socket_path.display()),
    ]
}
