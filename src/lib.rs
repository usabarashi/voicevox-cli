//! # VOICEVOX CLI
//!
//! A production-ready command-line text-to-speech synthesis tool using VOICEVOX Core 0.16.0.
//! Provides a macOS `say` command-compatible interface for Japanese TTS with various character voices.
//!
//! ## Architecture
//!
//! The tool uses a **daemon-client architecture** for optimal performance:
//! - **`voicevox-daemon`**: Background process with pre-loaded voice models
//! - **`voicevox-say`**: Lightweight client that communicates via Unix sockets
//!
//! ## Key Features
//!
//! - **Dynamic Voice Detection**: Zero hardcoded voice mappings - automatically adapts to available models
//! - **Functional Programming Design**: Immutable data structures, monadic composition, and declarative processing
//! - **High-Performance Architecture**: Optimized for minimal latency with pre-loaded models in daemon
//! - **macOS Integration**: Complete compatibility with macOS `say` command interface
//! - **Static Linking Priority**: VOICEVOX Core, ONNX Runtime, and OpenJTalk statically linked
//!
//! ## Module Organization
//!
//! This library follows **Rust 2018+ patterns** for optimal code organization:
//!
//! **Single-File Modules** (Simple, self-contained functionality):
//! - [`core`] - VOICEVOX Core wrapper with functional programming patterns
//! - [`voice`] - Dynamic voice detection and resolution system  
//! - [`paths`] - XDG-compliant path discovery and management
//! - [`setup`] - First-run setup and model management utilities
//! - [`ipc`] - Inter-process communication protocols and data structures
//!
//! **Multi-File Modules** (Complex functionality requiring separation):
//! - [`client`] - Client-side functionality (daemon communication, downloads, audio)
//! - [`daemon`] - Server-side functionality (model loading, synthesis, process management)
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use voicevox_cli::{VoicevoxCore, scan_available_models};
//!
//! // Initialize VOICEVOX Core with static linking
//! let mut core = VoicevoxCore::new()?;
//!
//! // Discover available voice models dynamically
//! let models = scan_available_models()?;
//!
//! // Load a voice model and synthesize speech
//! if let Some(model) = models.first() {
//!     core.load_model(&model.file_path)?;
//!     let audio = core.synthesize("こんにちは、ずんだもんなのだ", 3)?;
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

// Single-file modules (Rust 2018+ pattern)

/// VOICEVOX Core wrapper with functional programming patterns
pub mod core;

/// Inter-process communication protocols and data structures  
pub mod ipc;

/// Dynamic voice detection and resolution system
pub mod voice;

/// XDG-compliant path discovery and management
pub mod paths;

/// First-run setup and model management utilities
pub mod setup;

// Multi-file modules (complex functionality)

/// Client-side functionality (daemon client, download management)
pub mod client;

/// Server-side functionality (model loading, synthesis)
pub mod daemon;

// Re-export core types for convenient access
pub use core::VoicevoxCore;
pub use ipc::{DaemonRequest, DaemonResponse, SynthesizeOptions};
pub use paths::{find_models_dir, find_models_dir_client, find_openjtalk_dict, get_socket_path};
pub use setup::{attempt_first_run_setup, is_valid_models_directory};
pub use voice::{
    get_model_for_voice_id, resolve_voice_dynamic, scan_available_models, AvailableModel, Speaker,
    Style,
};
