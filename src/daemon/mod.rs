//! Server-side daemon functionality for voice synthesis
//!
//! This module implements the background daemon process that pre-loads voice models
//! and handles synthesis requests via Unix socket IPC. Designed for high performance
//! with instant response times after initial setup.

/// Background server implementation with model management
pub mod server;

/// Process management and duplicate prevention
pub mod process;

/// Zero-copy audio streaming support
pub mod streaming;

/// File descriptor passing for zero-copy transfer
pub mod fd_passing;

/// FD passing server with stream reuse pattern
pub mod fd_server;

pub use process::check_and_prevent_duplicate;
pub use server::{handle_client, run_daemon, run_daemon_with_config, DaemonState};
pub use streaming::SharedAudioBuffer;

#[cfg(test)]
mod tests {
    use super::server::ModelCache;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_model_cache_basic() {
        let mut cache = ModelCache::new(5);

        // Test initial state
        assert_eq!(cache.loaded_models.len(), 0);
        assert!(!cache.is_loaded(3));

        // Add models
        cache.add_model(3);
        assert!(cache.is_loaded(3));
        assert_eq!(cache.loaded_models.len(), 1);

        // Mark as used
        cache.mark_used(3);
        assert_eq!(*cache.usage_stats.get(&3).unwrap(), 1);

        // Mark as used again
        cache.mark_used(3);
        assert_eq!(*cache.usage_stats.get(&3).unwrap(), 2);
    }

    #[test]
    fn test_model_cache_lru_eviction() {
        let mut cache = ModelCache::new(5);

        // Load initial favorites
        for id in [3, 2, 8] {
            cache.add_model(id);
            thread::sleep(Duration::from_millis(10)); // Ensure different timestamps
        }

        // Load additional models
        for id in [10, 11] {
            cache.add_model(id);
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(cache.loaded_models.len(), 5);
        assert!(cache.should_evict());

        // Get LRU model (should be 10, as it's the oldest non-favorite)
        let lru = cache.get_lru_model();
        assert_eq!(lru, Some(10));

        // Remove LRU and add new model
        cache.remove_model(10);
        cache.add_model(12);

        assert_eq!(cache.loaded_models.len(), 5);
        assert!(!cache.is_loaded(10));
        assert!(cache.is_loaded(12));
    }

    #[test]
    fn test_favorites_not_evicted() {
        let mut cache = ModelCache::new(3);

        // Fill cache with favorites
        for id in [3, 2, 8] {
            cache.add_model(id);
        }

        // Try to get LRU - should be None as all are favorites
        assert_eq!(cache.get_lru_model(), None);

        // Add non-favorite
        cache.remove_model(8); // Make room
        cache.add_model(10);

        // Now LRU should return the non-favorite
        assert_eq!(cache.get_lru_model(), Some(10));
    }

    #[test]
    fn test_usage_statistics() {
        let mut cache = ModelCache::new(5);

        // Add and use models
        cache.add_model(3);
        cache.add_model(10);

        cache.mark_used(3);
        cache.mark_used(3);
        cache.mark_used(10);

        assert_eq!(*cache.usage_stats.get(&3).unwrap(), 2);
        assert_eq!(*cache.usage_stats.get(&10).unwrap(), 1);

        // Test stats string
        let stats = cache.get_stats();
        assert!(stats.contains("Loaded: 2 models"));
        assert!(stats.contains("Max: 5"));
    }

    #[test]
    fn test_model_cache_with_real_scenario() {
        let mut cache = ModelCache::new(5);

        println!("=== ModelCache Unit Test - Real Scenario ===");

        // Simulate daemon startup
        println!("1. Loading favorites on startup:");
        for id in [3, 2, 8] {
            cache.add_model(id);
            println!("   Loaded model {}", id);
            thread::sleep(Duration::from_millis(10));
        }

        // Simulate user requests
        let requests = vec![
            (3, "Zundamon - already loaded"),
            (10, "New model 10"),
            (11, "New model 11 - at limit"),
            (12, "New model 12 - should evict 10"),
        ];

        println!("\n2. Processing requests:");
        for (model_id, description) in requests {
            println!("   Request: {} (model {})", description, model_id);

            if !cache.is_loaded(model_id) {
                if cache.should_evict() {
                    if let Some(lru) = cache.get_lru_model() {
                        println!("     Evicting model {} (LRU)", lru);
                        cache.remove_model(lru);
                    }
                }
                cache.add_model(model_id);
                println!("     Loaded model {}", model_id);
            }

            cache.mark_used(model_id);
            thread::sleep(Duration::from_millis(10));
        }

        println!("\n3. Final state:");
        println!("   {}", cache.get_stats());

        // Verify favorites are still there
        assert!(cache.is_loaded(3));
        assert!(cache.is_loaded(2));
        assert!(cache.is_loaded(8));

        // Verify LRU worked
        assert!(!cache.is_loaded(10)); // Should have been evicted
        assert!(cache.is_loaded(12)); // Should be loaded
    }
}
