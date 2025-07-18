use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub text_splitter: TextSplitterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSplitterConfig {
    #[serde(default = "default_delimiters")]
    pub delimiters: Vec<String>,
    #[serde(default = "default_max_length")]
    pub max_length: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            text_splitter: TextSplitterConfig::default(),
        }
    }
}

impl Default for TextSplitterConfig {
    fn default() -> Self {
        Self {
            delimiters: default_delimiters(),
            max_length: default_max_length(),
        }
    }
}

fn default_delimiters() -> Vec<String> {
    vec![
        "。".to_string(),
        "！".to_string(),
        "？".to_string(),
        "．".to_string(),
        "\n".to_string(),
    ]
}

fn default_max_length() -> usize {
    100
}

impl Config {
    pub fn load() -> Result<Self> {
        if let Some(config_path) = Self::config_path()? {
            if config_path.exists() {
                let content = fs::read_to_string(&config_path)
                    .with_context(|| format!("Failed to read config from {:?}", config_path))?;
                let config: Config = toml::from_str(&content)
                    .with_context(|| format!("Failed to parse config from {:?}", config_path))?;
                Ok(config)
            } else {
                Ok(Self::default())
            }
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        if let Some(config_path) = Self::config_path()? {
            if let Some(parent) = config_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(self)?;
            fs::write(&config_path, content)
                .with_context(|| format!("Failed to write config to {:?}", config_path))?;
        }
        Ok(())
    }

    fn config_path() -> Result<Option<PathBuf>> {
        if let Ok(home) = std::env::var("HOME") {
            let config_dir = Path::new(&home).join(".config").join("voicevox-cli");
            Ok(Some(config_dir.join("config.toml")))
        } else {
            Ok(None)
        }
    }

    pub fn create_default_config_if_not_exists() -> Result<()> {
        if let Some(config_path) = Self::config_path()? {
            if !config_path.exists() {
                let default_config = Self::default();
                default_config.save()?;
                println!("Created default config at: {:?}", config_path);
            }
        }
        Ok(())
    }
}