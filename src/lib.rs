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

/// Client-side functionality (daemon client, download management)
pub mod client;

/// Server-side functionality (model loading, synthesis)
pub mod daemon;

// Re-export core types for convenient access
pub use core::VoicevoxCore;
pub use ipc::{DaemonRequest, DaemonResponse, SynthesizeOptions};
pub use voice::{Speaker, Style, get_model_for_voice_id, resolve_voice_dynamic, scan_available_models, AvailableModel};
pub use paths::{get_socket_path, find_models_dir, find_models_dir_client, find_openjtalk_dict};
pub use setup::{attempt_first_run_setup, is_valid_models_directory};