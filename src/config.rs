//! Configuration management for VOICEVOX CLI
//! 
//! Provides configuration file support for memory management and model preferences

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Memory management settings
    pub memory: MemoryConfig,
    
    /// Model preferences
    pub models: ModelConfig,
    
    /// Daemon settings
    pub daemon: DaemonConfig,
}

/// Memory management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    /// Maximum number of models to keep in memory
    pub max_loaded_models: usize,
    
    /// Enable LRU cache management
    pub enable_lru_cache: bool,
    
    /// Memory limit in MB (informational only)
    pub memory_limit_mb: Option<usize>,
}

/// Model preferences configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConfig {
    /// Models to preload on startup
    pub preload: Vec<u32>,
    
    /// Models that should never be unloaded (favorites)
    pub favorites: HashSet<u32>,
    
    /// Enable predictive preloading based on usage patterns
    pub predictive_preload: bool,
}

/// Daemon configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    /// Socket path for IPC
    pub socket_path: Option<PathBuf>,
    
    /// Auto-start timeout in seconds
    pub startup_timeout: u64,
    
    /// Enable debug logging
    pub debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            memory: MemoryConfig::default(),
            models: ModelConfig::default(),
            daemon: DaemonConfig::default(),
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_loaded_models: 5,
            enable_lru_cache: true,
            memory_limit_mb: None,
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            preload: vec![0, 1, 8], // Metan(0), Zundamon(1), Tsumugi(8)
            favorites: HashSet::from([0, 1, 8]),
            predictive_preload: false,
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            startup_timeout: 10,
            debug: false,
        }
    }
}

impl Config {
    /// Load configuration from file
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;
        
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }
    
    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;
        
        // Ensure config directory exists
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        
        Ok(())
    }
    
    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        
        Ok(config_dir.join("voicevox").join("config.toml"))
    }
    
    /// Create example configuration file
    pub fn create_example() -> Result<()> {
        let example = Config {
            memory: MemoryConfig {
                max_loaded_models: 5,
                enable_lru_cache: true,
                memory_limit_mb: Some(1024),
            },
            models: ModelConfig {
                preload: vec![3, 2, 8, 1], // Add Zundamon sweet variant
                favorites: HashSet::from([3, 2, 8]),
                predictive_preload: false,
            },
            daemon: DaemonConfig {
                socket_path: None,
                startup_timeout: 10,
                debug: false,
            },
        };
        
        let config_path = Self::config_path()?;
        let example_path = config_path.with_file_name("config.example.toml");
        
        // Ensure directory exists
        if let Some(parent) = example_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(&example)?;
        let content_with_comments = format!(
            "# VOICEVOX CLI Configuration File\n\
             # Copy this file to 'config.toml' and modify as needed\n\n\
             {}\n\n\
             # Memory Management\n\
             # max_loaded_models: Maximum number of voice models to keep in memory\n\
             # enable_lru_cache: Enable automatic unloading of least recently used models\n\
             # memory_limit_mb: Informational memory limit (not enforced)\n\n\
             # Model Preferences\n\
             # preload: Model IDs to load on daemon startup\n\
             # favorites: Model IDs that should never be unloaded\n\
             # predictive_preload: Enable predictive model loading (experimental)\n\n\
             # Daemon Settings\n\
             # socket_path: Custom Unix socket path (default: XDG runtime dir)\n\
             # startup_timeout: Timeout for daemon startup in seconds\n\
             # debug: Enable debug logging\n",
            content
        );
        
        std::fs::write(&example_path, content_with_comments)?;
        println!("Example configuration created at: {}", example_path.display());
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.memory.max_loaded_models, 5);
        assert!(config.memory.enable_lru_cache);
        assert_eq!(config.models.preload, vec![0, 1, 8]);
        assert_eq!(config.models.favorites.len(), 3);
    }
    
    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();
        
        assert_eq!(config.memory.max_loaded_models, deserialized.memory.max_loaded_models);
        assert_eq!(config.models.preload, deserialized.models.preload);
    }
}