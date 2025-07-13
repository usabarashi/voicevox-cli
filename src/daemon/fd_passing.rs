use anyhow::{Context, Result};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};

#[cfg(target_os = "linux")]
pub fn create_anonymous_file(name: &str, size: usize) -> Result<RawFd> {
    use tempfile::tempfile;

    let file = tempfile().context("Failed to create temporary file")?;

    file.set_len(size as u64)
        .context("Failed to set file size")?;

    let fd = file.as_raw_fd();
    std::mem::forget(file);

    Ok(fd)
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
pub fn create_anonymous_file(_name: &str, size: usize) -> Result<RawFd> {
    use tempfile::tempfile;
    let file = tempfile().context("Failed to create temporary file")?;

    file.set_len(size as u64)
        .context("Failed to set file size")?;

    let fd = file.as_raw_fd();
    std::mem::forget(file);

    Ok(fd)
}

pub struct AnonymousBuffer {
    fd: RawFd,
    size: usize,
}

impl AnonymousBuffer {
    pub fn new(name: &str, size: usize) -> Result<Self> {
        let fd = create_anonymous_file(name, size)?;
        Ok(Self { fd, size })
    }

    pub fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        let mut file = unsafe { std::fs::File::from_raw_fd(self.fd) };
        file.write_all(data)?;
        std::mem::forget(file);
        Ok(())
    }

    pub fn into_fd(self) -> RawFd {
        let fd = self.fd;
        std::mem::forget(self);
        fd
    }

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

        let _fd = buffer.into_fd();
    }
}
