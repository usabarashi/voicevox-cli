//! File descriptor passing support for zero-copy audio transfer

use anyhow::{Context, Result};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

/// Platform-specific anonymous memory file creation
#[cfg(target_os = "linux")]
pub fn create_anonymous_file(name: &str, size: usize) -> Result<RawFd> {
    // Linux implementation using memfd_create
    // For now, use tempfile fallback since nix doesn't expose memfd_create in stable API
    use tempfile::tempfile;

    let file = tempfile().context("Failed to create temporary file")?;

    file.set_len(size as u64)
        .context("Failed to set file size")?;

    let fd = file.as_raw_fd();
    std::mem::forget(file);

    Ok(fd)
}

/// Platform-specific anonymous memory file creation (macOS/BSD)
#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub fn create_anonymous_file(_name: &str, size: usize) -> Result<RawFd> {
    use tempfile::tempfile;

    // On macOS, use a temporary file that's immediately unlinked
    let file = tempfile().context("Failed to create temporary file")?;

    // Set the size
    file.set_len(size as u64)
        .context("Failed to set file size")?;

    let fd = file.as_raw_fd();
    // Prevent the file from being closed when dropped
    std::mem::forget(file);

    Ok(fd)
}

/// Anonymous memory buffer for zero-copy transfer
pub struct AnonymousBuffer {
    fd: RawFd,
    size: usize,
}

impl AnonymousBuffer {
    /// Create a new anonymous buffer
    pub fn new(name: &str, size: usize) -> Result<Self> {
        let fd = create_anonymous_file(name, size)?;
        Ok(Self { fd, size })
    }

    /// Write data to the buffer
    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        let mut file = unsafe { std::fs::File::from_raw_fd(self.fd) };
        file.write_all(data)?;
        // Prevent the file from being closed when dropped
        std::mem::forget(file);
        Ok(())
    }

    /// Get the file descriptor (transfers ownership)
    pub fn into_fd(self) -> RawFd {
        let fd = self.fd;
        std::mem::forget(self); // Prevent Drop from closing the fd
        fd
    }

    /// Get the size of the buffer
    pub fn size(&self) -> usize {
        self.size
    }
}

impl Drop for AnonymousBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

/// Send a file descriptor over a Unix socket
#[cfg(unix)]
pub fn send_fd(socket_fd: RawFd, fd_to_send: RawFd, metadata: &[u8]) -> Result<()> {
    use nix::sys::socket::{sendmsg, ControlMessage, MsgFlags};
    use std::io::IoSlice;

    let iov = [IoSlice::new(metadata)];
    let fds = [fd_to_send];
    let cmsg = [ControlMessage::ScmRights(&fds)];

    sendmsg::<()>(socket_fd, &iov, &cmsg, MsgFlags::empty(), None)
        .context("Failed to send file descriptor")?;

    Ok(())
}

/// Receive a file descriptor from a Unix socket
#[cfg(unix)]
pub fn receive_fd(socket_fd: RawFd, metadata_buf: &mut [u8]) -> Result<(RawFd, usize)> {
    use nix::sys::socket::{recvmsg, ControlMessageOwned, MsgFlags};
    use std::io::IoSliceMut;

    let mut iov = [IoSliceMut::new(metadata_buf)];
    let mut cmsgspace = nix::cmsg_space!(RawFd);

    let msg = recvmsg::<()>(socket_fd, &mut iov, Some(&mut cmsgspace), MsgFlags::empty())
        .context("Failed to receive message")?;

    let received_fd = msg
        .cmsgs()
        .ok()
        .and_then(|mut cmsgs| {
            cmsgs.find_map(|cmsg| {
                if let ControlMessageOwned::ScmRights(fds) = cmsg {
                    fds.first().copied()
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| anyhow::anyhow!("No file descriptor received"))?;

    Ok((received_fd, msg.bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn test_anonymous_buffer() {
        let data = b"Hello, zero-copy world!";
        let mut buffer =
            AnonymousBuffer::new("test_buffer", data.len()).expect("Failed to create buffer");

        buffer.write_all(data).expect("Failed to write data");
        assert_eq!(buffer.size(), data.len());

        // In a real test, we'd pass this FD to another process
        let _fd = buffer.into_fd();
    }
}
