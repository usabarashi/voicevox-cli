//! Configuration management for VOICEVOX CLI
//!
//! Provides configuration file support for daemon settings

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Daemon settings
    pub daemon: DaemonConfig,
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
        crate::paths::ensure_parent_dir(&config_path)?;

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
            daemon: DaemonConfig {
                socket_path: None,
                startup_timeout: 10,
                debug: false,
            },
        };

        let config_path = Self::config_path()?;
        let example_path = config_path.with_file_name("config.example.toml");

        // Ensure directory exists
        crate::paths::ensure_parent_dir(&example_path)?;

        let content = toml::to_string_pretty(&example)?;
        let content_with_comments = format!(
            "# VOICEVOX CLI Configuration File\n\
             # Copy this file to 'config.toml' and modify as needed\n\n\
             {}\n\n\
             # Daemon Settings\n\
             # socket_path: Custom Unix socket path (default: XDG runtime dir)\n\
             # startup_timeout: Timeout for daemon startup in seconds\n\
             # debug: Enable debug logging\n",
            content
        );

        std::fs::write(&example_path, content_with_comments)?;
        println!(
            "Example configuration created at: {}",
            example_path.display()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.daemon.startup_timeout, 10);
        assert!(!config.daemon.debug);
        assert!(config.daemon.socket_path.is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let serialized = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(
            config.daemon.startup_timeout,
            deserialized.daemon.startup_timeout
        );
        assert_eq!(config.daemon.debug, deserialized.daemon.debug);
    }
}
