//! VOICEVOX CLI - Japanese text-to-speech using VOICEVOX Core

/// VOICEVOX Core wrapper
pub mod core;

/// Inter-process communication protocols
pub mod ipc;

/// Dynamic voice detection
pub mod voice;

/// Path discovery and management
pub mod paths;

/// First-run setup and model management
pub mod setup;

/// Client-side functionality
pub mod client;

/// Server-side functionality
pub mod daemon;

pub use core::{VoicevoxCore, CoreSynthesis, CoreConfig};
pub use ipc::{
    DaemonRequest, DaemonResponse, SynthesizeOptions, IpcConfig,
    OwnedRequest, OwnedResponse, OwnedSynthesizeOptions
};
pub use paths::{find_models_dir, find_models_dir_client, find_openjtalk_dict, get_socket_path};
pub use setup::{attempt_first_run_setup, is_valid_models_directory};
pub use voice::{
    get_model_for_voice_id, resolve_voice_dynamic, scan_available_models, AvailableModel, Speaker,
    Style,
};

pub mod error {
    use std::fmt;
    
    #[derive(Debug)]
    pub enum VoicevoxError {
        SynthesisError {
            text: String,
            style_id: u32,
            source: anyhow::Error,
        },
        ModelError {
            model_path: std::path::PathBuf,
            operation: String,
            source: anyhow::Error,
        },
        IpcError {
            endpoint: String,
            operation: String,
            source: anyhow::Error,
        },
        VoiceResolutionError {
            input: String,
            available_options: Vec<String>,
            source: anyhow::Error,
        },
        ConfigError {
            field: String,
            value: String,
            constraint: String,
        },
        Other(anyhow::Error),
    }
    
    impl fmt::Display for VoicevoxError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                VoicevoxError::SynthesisError { text, style_id, source } => {
                    write!(f, "Synthesis failed for style_id {} (text length: {}): {}", 
                           style_id, text.len(), source)
                }
                VoicevoxError::ModelError { model_path, operation, source } => {
                    write!(f, "Model {} operation '{}': {}", 
                           model_path.display(), operation, source)
                }
                VoicevoxError::IpcError { endpoint, operation, source } => {
                    write!(f, "IPC {} operation '{}': {}", endpoint, operation, source)
                }
                VoicevoxError::VoiceResolutionError { input, available_options, source } => {
                    write!(f, "Voice resolution failed for '{}' (available: {}): {}", 
                           input, available_options.join(", "), source)
                }
                VoicevoxError::ConfigError { field, value, constraint } => {
                    write!(f, "Configuration error: field '{}' value '{}' violates constraint '{}'", 
                           field, value, constraint)
                }
                VoicevoxError::Other(source) => write!(f, "{}", source),
            }
        }
    }
    
    impl std::error::Error for VoicevoxError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                VoicevoxError::SynthesisError { source, .. } |
                VoicevoxError::ModelError { source, .. } |
                VoicevoxError::IpcError { source, .. } |
                VoicevoxError::VoiceResolutionError { source, .. } |
                VoicevoxError::Other(source) => source.source(),
                VoicevoxError::ConfigError { .. } => None,
            }
        }
    }
    
    pub type Result<T> = std::result::Result<T, VoicevoxError>;
    
    pub trait ErrorExt<T> {
        fn with_synthesis_context(self, text: &str, style_id: u32) -> Result<T>;
        fn with_model_context(self, model_path: std::path::PathBuf, operation: &str) -> Result<T>;
        fn with_ipc_context(self, endpoint: &str, operation: &str) -> Result<T>;
        fn with_voice_context(self, input: &str, available: Vec<String>) -> Result<T>;
    }
    
    impl<T> ErrorExt<T> for anyhow::Result<T> {
        fn with_synthesis_context(self, text: &str, style_id: u32) -> Result<T> {
            self.map_err(|e| VoicevoxError::SynthesisError {
                text: text.to_string(),
                style_id,
                source: e,
            })
        }
        
        fn with_model_context(self, model_path: std::path::PathBuf, operation: &str) -> Result<T> {
            self.map_err(|e| VoicevoxError::ModelError {
                model_path,
                operation: operation.to_string(),
                source: e,
            })
        }
        
        fn with_ipc_context(self, endpoint: &str, operation: &str) -> Result<T> {
            self.map_err(|e| VoicevoxError::IpcError {
                endpoint: endpoint.to_string(),
                operation: operation.to_string(),
                source: e,
            })
        }
        
        fn with_voice_context(self, input: &str, available: Vec<String>) -> Result<T> {
            self.map_err(|e| VoicevoxError::VoiceResolutionError {
                input: input.to_string(),
                available_options: available,
                source: e,
            })
        }
    }
}
