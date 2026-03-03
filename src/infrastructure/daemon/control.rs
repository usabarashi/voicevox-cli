use std::path::Path;
use std::process::Command;

fn allow_unsafe_path_commands() -> bool {
    std::env::var_os("VOICEVOX_ALLOW_UNSAFE_PATH_COMMANDS").is_some()
}

fn system_command_path(preferred: &'static str, fallback_name: &'static str) -> &'static str {
    if Path::new(preferred).is_file() {
        preferred
    } else if allow_unsafe_path_commands() {
        fallback_name
    } else {
        preferred
    }
}

#[must_use]
pub fn is_socket_responsive(socket_path: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket_path).is_ok()
}

#[must_use]
pub fn pid_memory_info_line(pid_num: u32) -> Option<String> {
    let ps_output = Command::new(system_command_path("/bin/ps", "ps"))
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
    Command::new(system_command_path("/bin/kill", "kill"))
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .is_ok_and(|status| status.success())
}
