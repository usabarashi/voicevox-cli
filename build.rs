use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// Dependencies are managed by Nix with fixed hashes for reproducibility

fn main() {
    let _out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let current_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = PathBuf::from(current_dir);

    // Check for VOICEVOX Core - should be provided by Nix or manually placed
    let voicevox_core_dir = src_dir.join("voicevox_core");
    if !voicevox_core_dir.exists() {
        println!("cargo:warning=VOICEVOX Core not found. Using Nix build or place manually in voicevox_core/");
        println!("cargo:warning=Expected structure: voicevox_core/lib/libvoicevox_core.dylib");
        // Don't error out - let Nix handle dependency management
    }

    // Tell cargo to look for shared libraries in the specified directory
    let lib_path = src_dir.join("voicevox_core/lib");

    println!("cargo:rustc-link-search=native={}", lib_path.display());
    
    // Set runtime path for dylib loading
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());

    // Link VOICEVOX libraries by full path
    let dylib_path = lib_path.join("libvoicevox_core.dylib");
    println!("cargo:rustc-link-arg={}", dylib_path.display());

    // Tell cargo to invalidate the built crate whenever the header changes
    let header_path = src_dir.join("voicevox_core/include/voicevox_core.h");
    println!("cargo:rerun-if-changed={}", header_path.display());
    println!("cargo:rerun-if-changed=build.rs");

    // Only generate bindings if the feature is enabled
    #[cfg(feature = "use_bindgen")]
    {
        use bindgen;

        let bindings = bindgen::Builder::default()
            .header(&header_path.to_string_lossy())
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .allowlist_function("voicevox_.*")
            .allowlist_type("Voicevox.*")
            .allowlist_var("VOICEVOX_.*")
            .generate()
            .expect("Unable to generate bindings");

        bindings
            .write_to_file(out_dir.join("bindings.rs"))
            .expect("Couldn't write bindings!");
    }
}

// Build script simplified - Nix handles dependency management
