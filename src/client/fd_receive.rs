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
