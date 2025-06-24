// Build script for VOICEVOX TTS
// Using native Rust VOICEVOX Core implementation - no FFI linking required

fn main() {
    // Native Rust implementation uses voicevox_core crate directly
    // No dynamic library linking needed for development builds
    
    // For production builds, VOICEVOX Core libraries are managed by:
    // 1. Nix build system (pre-configured paths)
    // 2. Runtime downloader (voicevox-download)
    
    println!("cargo:rerun-if-changed=build.rs");
}