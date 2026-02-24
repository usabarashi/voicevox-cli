use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

fn write_line(mut writer: impl Write, message: &str) {
    let _ = writeln!(writer, "{message}");
}

pub fn log(level: LogLevel, message: &str) {
    match level {
        LogLevel::Info => write_line(io::stdout(), message),
        LogLevel::Warn | LogLevel::Error => write_line(io::stderr(), message),
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
