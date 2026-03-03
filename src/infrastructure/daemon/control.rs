use std::path::Path;
use std::process::Command;

#[must_use]
pub fn is_socket_responsive(socket_path: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket_path).is_ok()
}

#[must_use]
pub fn pid_memory_info_line(pid_num: u32) -> Option<String> {
    let ps_output = Command::new(crate::config::command_path_or_fallback(
        crate::config::SYSTEM_PS_PATH,
        "ps",
    ))
    .args(["-p", &pid_num.to_string(), "-o", "rss,pmem,time"])
    .output()
    .ok()?;

    if !ps_output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&ps_output.stdout)
        .lines()
        .nth(1)
        .map(str::trim)
        .map(ToOwned::to_owned)
}

#[must_use]
pub fn terminate_process(pid: u32) -> bool {
    Command::new(crate::config::command_path_or_fallback(
        crate::config::SYSTEM_KILL_PATH,
        "kill",
    ))
    .arg("-TERM")
    .arg(pid.to_string())
    .status()
    .is_ok_and(|status| status.success())
}
