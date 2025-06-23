use std::env;
use std::path::PathBuf;

fn main() {
    let _out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Get the current source directory
    let current_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = PathBuf::from(current_dir);

    // Tell cargo to look for shared libraries in the specified directory
    let lib_path = src_dir.join("voicevox_core/c_api/lib");
    let onnx_lib_path = src_dir.join("voicevox_core/onnxruntime/lib");

    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-search=native={}", onnx_lib_path.display());

    // Only link VOICEVOX libraries if the feature is enabled
    #[cfg(feature = "link_voicevox")]
    {
        println!("cargo:rustc-link-lib=dylib=voicevox_core");
        println!("cargo:rustc-link-lib=dylib=voicevox_onnxruntime.1.17.3");
    }

    // Tell cargo to invalidate the built crate whenever the header changes
    let header_path = src_dir.join("voicevox_core/c_api/include/voicevox_core.h");
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
