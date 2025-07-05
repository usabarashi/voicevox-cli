//! Zero-copy audio streaming support for memory efficiency

use bytes::{Bytes, BytesMut};
use std::io::{self, Write};
use std::sync::Arc;

/// Shared immutable audio buffer for zero-copy operations
#[derive(Clone)]
pub struct SharedAudioBuffer {
    data: Arc<Bytes>,
}

impl SharedAudioBuffer {
    /// Create a new shared buffer from WAV data
    pub fn new(wav_data: Vec<u8>) -> Self {
        Self {
            data: Arc::new(Bytes::from(wav_data)),
        }
    }

    /// Get a reference to the underlying bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get the size of the buffer
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Create a slice view without copying
    pub fn slice(&self, start: usize, end: usize) -> Bytes {
        self.data.slice(start..end)
    }

    /// Get chunks for streaming
    pub fn chunks(&self, chunk_size: usize) -> AudioChunkIterator {
        AudioChunkIterator {
            buffer: self.clone(),
            position: 0,
            chunk_size,
        }
    }
}

/// Iterator for streaming audio chunks
pub struct AudioChunkIterator {
    buffer: SharedAudioBuffer,
    position: usize,
    chunk_size: usize,
}

impl Iterator for AudioChunkIterator {
    type Item = Bytes;

    fn next(&mut self) -> Option<Self::Item> {
        if self.position >= self.buffer.len() {
            return None;
        }

        let end = (self.position + self.chunk_size).min(self.buffer.len());
        let chunk = self.buffer.slice(self.position, end);
        self.position = end;

        Some(chunk)
    }
}

/// Memory pool for reusing buffers
pub struct AudioBufferPool {
    buffers: Vec<BytesMut>,
    max_size: usize,
}

impl AudioBufferPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffers: Vec::with_capacity(4),
            max_size,
        }
    }

    /// Get a buffer from the pool or create a new one
    pub fn get(&mut self) -> BytesMut {
        self.buffers
            .pop()
            .unwrap_or_else(|| BytesMut::with_capacity(self.max_size))
    }

    /// Return a buffer to the pool
    pub fn put(&mut self, mut buffer: BytesMut) {
        if self.buffers.len() < 4 && buffer.capacity() <= self.max_size {
            buffer.clear();
            self.buffers.push(buffer);
        }
    }
}

/// Write audio data efficiently without multiple copies
pub fn write_audio_efficient<W: Write>(
    writer: &mut W,
    audio_buffer: &SharedAudioBuffer,
    chunk_size: usize,
) -> io::Result<()> {
    for chunk in audio_buffer.chunks(chunk_size) {
        writer.write_all(&chunk)?;
    }
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_buffer() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let buffer = SharedAudioBuffer::new(data);

        assert_eq!(buffer.len(), 8);
        assert_eq!(buffer.as_bytes(), &[1, 2, 3, 4, 5, 6, 7, 8]);

        let slice = buffer.slice(2, 6);
        assert_eq!(&slice[..], &[3, 4, 5, 6]);
    }

    #[test]
    fn test_chunk_iterator() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let buffer = SharedAudioBuffer::new(data);

        let chunks: Vec<_> = buffer.chunks(3).collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!(&chunks[0][..], &[1, 2, 3]);
        assert_eq!(&chunks[1][..], &[3, 4, 5]);
        assert_eq!(&chunks[2][..], &[7, 8]);
    }
}
