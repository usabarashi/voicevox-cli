//! Memory pool for efficient audio buffer management

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Thread-safe memory pool for reusing audio buffers
#[derive(Clone)]
pub struct AudioMemoryPool {
    pool: Arc<Mutex<MemoryPoolInner>>,
}

struct MemoryPoolInner {
    buffers: VecDeque<Vec<u8>>,
    max_buffers: usize,
    buffer_capacity: usize,
}

impl AudioMemoryPool {
    /// Create a new memory pool
    pub fn new(max_buffers: usize, buffer_capacity: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(MemoryPoolInner {
                buffers: VecDeque::with_capacity(max_buffers),
                max_buffers,
                buffer_capacity,
            })),
        }
    }

    /// Get a buffer from the pool or create a new one
    pub fn acquire(&self) -> PooledBuffer {
        let mut pool = self.pool.lock().unwrap();

        let buffer = pool
            .buffers
            .pop_front()
            .unwrap_or_else(|| Vec::with_capacity(pool.buffer_capacity));

        PooledBuffer {
            buffer,
            pool: self.clone(),
        }
    }

    /// Return a buffer to the pool
    fn return_buffer(&self, mut buffer: Vec<u8>) {
        let mut pool = self.pool.lock().unwrap();

        // Only return to pool if under limit and reasonable size
        if pool.buffers.len() < pool.max_buffers && buffer.capacity() <= pool.buffer_capacity * 2 {
            buffer.clear();
            pool.buffers.push_back(buffer);
        }
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        let pool = self.pool.lock().unwrap();
        PoolStats {
            available_buffers: pool.buffers.len(),
            max_buffers: pool.max_buffers,
            buffer_capacity: pool.buffer_capacity,
        }
    }
}

/// A buffer borrowed from the pool
pub struct PooledBuffer {
    buffer: Vec<u8>,
    pool: AudioMemoryPool,
}

impl PooledBuffer {
    /// Get a mutable reference to the buffer
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buffer
    }

    /// Get an immutable reference to the buffer
    pub fn buffer(&self) -> &[u8] {
        &self.buffer
    }

    /// Take ownership of the buffer (won't be returned to pool)
    pub fn into_vec(mut self) -> Vec<u8> {
        let buffer = std::mem::take(&mut self.buffer);
        std::mem::forget(self); // Prevent Drop from returning to pool
        buffer
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        let buffer = std::mem::take(&mut self.buffer);
        self.pool.return_buffer(buffer);
    }
}

impl std::ops::Deref for PooledBuffer {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl std::ops::DerefMut for PooledBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub available_buffers: usize,
    pub max_buffers: usize,
    pub buffer_capacity: usize,
}

// Global audio memory pool instance
lazy_static::lazy_static! {
    pub static ref AUDIO_POOL: AudioMemoryPool = AudioMemoryPool::new(
        8,  // max 8 buffers in pool
        1024 * 1024 * 2  // 2MB buffer capacity
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool() {
        let pool = AudioMemoryPool::new(2, 1024);

        // Get buffer from pool
        let mut buffer1 = pool.acquire();
        buffer1.extend_from_slice(b"test data");
        assert_eq!(buffer1.buffer(), b"test data");

        // Get another buffer
        let buffer2 = pool.acquire();
        assert_eq!(buffer2.len(), 0); // Should be empty

        // Return buffers
        drop(buffer1);
        drop(buffer2);

        // Check pool stats
        let stats = pool.stats();
        assert_eq!(stats.available_buffers, 2);
    }

    #[test]
    fn test_buffer_reuse() {
        let pool = AudioMemoryPool::new(1, 1024);

        // First use
        {
            let mut buffer = pool.acquire();
            buffer.extend_from_slice(b"data");
        }

        // Buffer should be cleared when reused
        {
            let buffer = pool.acquire();
            assert_eq!(buffer.len(), 0);
        }
    }
}
