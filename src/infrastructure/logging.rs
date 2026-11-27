use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Set once by a process that speaks a line protocol on stdout.
static INFO_TO_STDERR: AtomicBool = AtomicBool::new(false);

/// Routes informational lines to stderr instead of stdout.
///
/// The MCP server writes JSON-RPC frames to stdout, where a stray log line is a
/// protocol violation. Code reachable from a tool call (daemon autostart,
/// synthesis) logs through this module, so the server opts in here once at
/// startup instead of threading a writer through every layer. The CLI leaves it
/// unset and keeps stdout for its human-facing output.
pub fn route_info_to_stderr() {
    INFO_TO_STDERR.store(true, Ordering::Relaxed);
}

/// Where a line of the given level is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Stdout,
    Stderr,
}

const fn target_for(level: LogLevel, info_to_stderr: bool) -> Target {
    match level {
        // Only informational lines are ever contested: the CLI wants them on
        // stdout, a line-protocol process needs them off it.
        LogLevel::Info if !info_to_stderr => Target::Stdout,
        LogLevel::Info | LogLevel::Warn | LogLevel::Error => Target::Stderr,
    }
}

fn write_line(mut writer: impl Write, message: &str) {
    let _ = writeln!(writer, "{message}");
}

pub fn log(level: LogLevel, message: &str) {
    match target_for(level, INFO_TO_STDERR.load(Ordering::Relaxed)) {
        Target::Stdout => write_line(io::stdout(), message),
        Target::Stderr => write_line(io::stderr(), message),
    }
}

pub fn info(message: &str) {
    log(LogLevel::Info, message);
}

pub fn warn(message: &str) {
    log(LogLevel::Warn, message);
}

pub fn error(message: &str) {
    log(LogLevel::Error, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_keeps_info_on_stdout() {
        assert_eq!(target_for(LogLevel::Info, false), Target::Stdout);
    }

    #[test]
    fn line_protocol_process_moves_info_off_stdout() {
        assert_eq!(target_for(LogLevel::Info, true), Target::Stderr);
    }

    #[test]
    fn warnings_and_errors_never_use_stdout() {
        for info_to_stderr in [false, true] {
            assert_eq!(target_for(LogLevel::Warn, info_to_stderr), Target::Stderr);
            assert_eq!(target_for(LogLevel::Error, info_to_stderr), Target::Stderr);
        }
    }
}
