//! Client-side file descriptor reception for zero-copy audio

use anyhow::{Context, Result};
use memmap2::MmapOptions;
#[cfg(unix)]
use std::os::unix::io::{FromRawFd, RawFd};

/// Received audio buffer via file descriptor
#[cfg(unix)]
pub struct ReceivedAudioBuffer {
    mmap: memmap2::Mmap,
    fd: RawFd,
}

#[cfg(unix)]
impl ReceivedAudioBuffer {
    /// Create from received file descriptor
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The file descriptor `fd` is valid and owned by the caller
    /// - The file descriptor refers to a memory-mapped file of at least `size` bytes
    /// - The file descriptor will not be used after this call (ownership is transferred)
    pub unsafe fn from_fd(fd: RawFd, size: usize) -> Result<Self> {
        let file = std::fs::File::from_raw_fd(fd);

        // Memory map the file
        let mmap = MmapOptions::new()
            .len(size)
            .map(&file)
            .context("Failed to memory map received audio")?;

        // Prevent file from being closed when dropped
        std::mem::forget(file);

        Ok(Self { mmap, fd })
    }

    /// Get the audio data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &self.mmap
    }

    /// Convert to owned Vec (copies data)
    pub fn to_vec(&self) -> Vec<u8> {
        self.mmap.to_vec()
    }
}

#[cfg(unix)]
impl Drop for ReceivedAudioBuffer {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

/// Handle zero-copy audio response
#[cfg(unix)]
pub async fn handle_audio_fd_response(
    socket: &tokio::net::UnixStream,
    size: usize,
    _format: crate::ipc::AudioFormat,
) -> Result<Vec<u8>> {
    use super::super::daemon::fd_passing::receive_fd;
    use std::os::unix::io::AsRawFd;

    // Prepare to receive FD
    let mut metadata_buf = vec![0u8; 16]; // Small metadata buffer
    let socket_fd = socket.as_raw_fd();

    // Receive the file descriptor
    let (received_fd, _metadata_size) = receive_fd(socket_fd, &mut metadata_buf)
        .context("Failed to receive audio file descriptor")?;

    // Map and read the audio data
    let audio_buffer = unsafe { ReceivedAudioBuffer::from_fd(received_fd, size)? };

    // For now, convert to Vec for compatibility
    // In future, could pass the mmap directly to audio playback
    Ok(audio_buffer.to_vec())
}

/// Client capability detection
pub fn supports_zero_copy() -> bool {
    #[cfg(unix)]
    {
        // Check if we're on a supported platform
        true
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_detection() {
        #[cfg(unix)]
        assert!(supports_zero_copy());

        #[cfg(not(unix))]
        assert!(!supports_zero_copy());
    }
}
