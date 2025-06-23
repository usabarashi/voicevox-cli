use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const VOICEVOX_CORE_VERSION: &str = "0.16.0";
const ONNXRUNTIME_VERSION: &str = "1.17.3";

fn main() {
    let _out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let current_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let src_dir = PathBuf::from(current_dir);

    // Download VOICEVOX Core if not present
    let voicevox_core_dir = src_dir.join("voicevox_core");
    if !voicevox_core_dir.exists() {
        download_voicevox_core(&src_dir);
        download_onnxruntime(&src_dir);
    }

    // Download voice models if not present
    let models_dir = src_dir.join("models");
    if !models_dir.exists() || !has_essential_models(&models_dir) {
        download_voice_models(&src_dir);
    }

    // Download OpenJTalk dictionary if not present
    let dict_dir = src_dir.join("dict");
    if !dict_dir.exists() || !is_valid_dict_directory(&dict_dir) {
        download_openjtalk_dict(&src_dir);
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

fn download_voicevox_core(src_dir: &Path) {
    // Get target architecture
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    
    // Only download for supported platforms
    if target_os != "macos" {
        println!("cargo:warning=VOICEVOX Core download only supported on macOS");
        return;
    }
    
    let arch_suffix = match target_arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        _ => {
            println!("cargo:warning=Unsupported architecture: {}", target_arch);
            return;
        }
    };
    
    let filename = format!("voicevox_core-osx-{}-{}.zip", arch_suffix, VOICEVOX_CORE_VERSION);
    let url = format!(
        "https://github.com/VOICEVOX/voicevox_core/releases/download/{}/{}",
        VOICEVOX_CORE_VERSION,
        filename
    );
    
    println!("cargo:warning=Downloading VOICEVOX Core from: {}", url);
    
    // Create temp directory
    let temp_dir = src_dir.join("temp_voicevox_download");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    
    let zip_path = temp_dir.join(&filename);
    
    // Download using curl (available on macOS by default)
    let output = Command::new("curl")
        .args(["-L", "-o", zip_path.to_str().unwrap(), &url])
        .output()
        .expect("Failed to execute curl");
    
    if !output.status.success() {
        panic!("Failed to download VOICEVOX Core: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Extract using unzip (available on macOS by default)
    let output = Command::new("unzip")
        .args(["-q", zip_path.to_str().unwrap(), "-d", temp_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute unzip");
    
    if !output.status.success() {
        panic!("Failed to extract VOICEVOX Core: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Find extracted directory and move to voicevox_core
    let extracted_dirs: Vec<_> = fs::read_dir(&temp_dir)
        .expect("Failed to read temp directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() && path.file_name()?.to_str()?.starts_with("voicevox_core") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    
    if extracted_dirs.is_empty() {
        panic!("No voicevox_core directory found in extracted files");
    }
    
    let extracted_dir = &extracted_dirs[0];
    let target_dir = src_dir.join("voicevox_core");
    fs::rename(extracted_dir, &target_dir).expect("Failed to move voicevox_core directory");
    
    // Fix the dylib install_name to use relative path
    let dylib_path = target_dir.join("lib/libvoicevox_core.dylib");
    let output = Command::new("install_name_tool")
        .args(["-id", "@rpath/libvoicevox_core.dylib", dylib_path.to_str().unwrap()])
        .output()
        .expect("Failed to fix dylib install_name");
    
    if !output.status.success() {
        println!("cargo:warning=Failed to fix dylib install_name: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Clean up
    fs::remove_dir_all(&temp_dir).expect("Failed to clean up temp directory");
    
    println!("cargo:warning=VOICEVOX Core {} downloaded and extracted successfully", VOICEVOX_CORE_VERSION);
}

fn download_onnxruntime(src_dir: &Path) {
    // Get target architecture
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    
    // Only download for supported platforms
    if target_os != "macos" {
        println!("cargo:warning=ONNX Runtime download only supported on macOS");
        return;
    }
    
    let arch_name = match target_arch.as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        _ => {
            println!("cargo:warning=Unsupported architecture for ONNX Runtime: {}", target_arch);
            return;
        }
    };
    
    let filename = format!("voicevox_onnxruntime-osx-{}-{}.tgz", arch_name, ONNXRUNTIME_VERSION);
    let url = format!(
        "https://github.com/VOICEVOX/onnxruntime-builder/releases/download/voicevox_onnxruntime-{}/{}",
        ONNXRUNTIME_VERSION,
        filename
    );
    
    println!("cargo:warning=Downloading ONNX Runtime from: {}", url);
    
    // Create temp directory
    let temp_dir = src_dir.join("temp_onnxruntime_download");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    
    let archive_path = temp_dir.join(&filename);
    
    // Download using curl
    let output = Command::new("curl")
        .args(["-L", "-o", archive_path.to_str().unwrap(), &url])
        .output()
        .expect("Failed to execute curl");
    
    if !output.status.success() {
        panic!("Failed to download ONNX Runtime: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Extract using tar
    let output = Command::new("tar")
        .args(["-xzf", archive_path.to_str().unwrap(), "-C", temp_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute tar");
    
    if !output.status.success() {
        panic!("Failed to extract ONNX Runtime: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Find extracted directory and copy to voicevox_core/lib
    let extracted_dirs: Vec<_> = fs::read_dir(&temp_dir)
        .expect("Failed to read temp directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() && path.file_name()?.to_str()?.starts_with("voicevox_onnxruntime") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    
    if extracted_dirs.is_empty() {
        panic!("No onnxruntime directory found in extracted files");
    }
    
    let extracted_dir = &extracted_dirs[0];
    let lib_dir = extracted_dir.join("lib");
    let target_lib_dir = src_dir.join("voicevox_core/lib");
    
    // Copy onnxruntime library files to voicevox_core/lib
    if lib_dir.exists() {
        for entry in fs::read_dir(&lib_dir).expect("Failed to read onnxruntime lib directory") {
            let entry = entry.expect("Failed to read directory entry");
            let src_path = entry.path();
            let dst_path = target_lib_dir.join(entry.file_name());
            
            if src_path.is_file() {
                fs::copy(&src_path, &dst_path).expect("Failed to copy onnxruntime library");
                
                // Fix install_name for dylib files
                if let Some(ext) = src_path.extension() {
                    if ext == "dylib" {
                        let output = Command::new("install_name_tool")
                            .args(["-id", &format!("@rpath/{}", entry.file_name().to_string_lossy()), dst_path.to_str().unwrap()])
                            .output()
                            .expect("Failed to fix onnxruntime dylib install_name");
                        
                        if !output.status.success() {
                            println!("cargo:warning=Failed to fix onnxruntime dylib install_name: {}", String::from_utf8_lossy(&output.stderr));
                        }
                    }
                }
            }
        }
    }
    
    // Clean up
    fs::remove_dir_all(&temp_dir).expect("Failed to clean up temp directory");
    
    println!("cargo:warning=ONNX Runtime {} downloaded and integrated successfully", ONNXRUNTIME_VERSION);
}

fn download_voice_models(src_dir: &Path) {
    println!("cargo:warning=Downloading essential VOICEVOX voice models...");
    
    // Essential models for basic functionality
    let essential_models = [
        "3.vvm",  // ずんだもん
        "2.vvm",  // 四国めたん
        "8.vvm",  // 春日部つむぎ
    ];
    
    let models_dir = src_dir.join("models");
    fs::create_dir_all(&models_dir).expect("Failed to create models directory");
    
    let mut downloaded_count = 0;
    
    for model_name in &essential_models {
        let model_url = format!(
            "https://github.com/VOICEVOX/voicevox_core/releases/download/0.16.0/{}",
            model_name
        );
        
        let model_path = models_dir.join(model_name);
        
        println!("cargo:warning=Downloading voice model: {}", model_name);
        
        let output = Command::new("curl")
            .args(["-L", "-o", model_path.to_str().unwrap(), &model_url])
            .output()
            .expect("Failed to execute curl");
        
        if output.status.success() {
            downloaded_count += 1;
            println!("cargo:warning=Successfully downloaded: {}", model_name);
        } else {
            println!("cargo:warning=Failed to download {}: {}", model_name, String::from_utf8_lossy(&output.stderr));
        }
    }
    
    println!("cargo:warning=Downloaded {} essential voice models", downloaded_count);
}

fn download_openjtalk_dict(src_dir: &Path) {
    println!("cargo:warning=Downloading OpenJTalk dictionary...");
    
    // OpenJTalk dictionary URL (using open-jtalk official release)
    let dict_url = "https://sourceforge.net/projects/open-jtalk/files/Dictionary/open_jtalk_dic-1.11/open_jtalk_dic_utf_8-1.11.tar.gz/download";
    
    let dict_dir = src_dir.join("dict");
    let temp_dir = src_dir.join("temp_dict_download");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp directory");
    
    let archive_path = temp_dir.join("open_jtalk_dic_utf_8-1.11.tar.gz");
    
    println!("cargo:warning=Downloading OpenJTalk dictionary from SourceForge...");
    
    // Download using curl with redirect following
    let output = Command::new("curl")
        .args(["-L", "-o", archive_path.to_str().unwrap(), dict_url])
        .output()
        .expect("Failed to execute curl");
    
    if !output.status.success() {
        panic!("Failed to download OpenJTalk dictionary: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Extract using tar
    let output = Command::new("tar")
        .args(["-xzf", archive_path.to_str().unwrap(), "-C", temp_dir.to_str().unwrap()])
        .output()
        .expect("Failed to execute tar");
    
    if !output.status.success() {
        panic!("Failed to extract OpenJTalk dictionary: {}", String::from_utf8_lossy(&output.stderr));
    }
    
    // Find extracted directory and move to dict
    let extracted_dirs: Vec<_> = fs::read_dir(&temp_dir)
        .expect("Failed to read temp directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() && path.file_name()?.to_str()?.starts_with("open_jtalk_dic") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    
    if extracted_dirs.is_empty() {
        panic!("No OpenJTalk dictionary directory found in extracted files");
    }
    
    let extracted_dir = &extracted_dirs[0];
    fs::rename(extracted_dir, &dict_dir).expect("Failed to move OpenJTalk dictionary");
    
    // Clean up
    fs::remove_dir_all(&temp_dir).expect("Failed to clean up temp directory");
    
    println!("cargo:warning=OpenJTalk dictionary downloaded and extracted successfully");
}

fn has_essential_models(models_dir: &Path) -> bool {
    let essential_models = ["3.vvm", "2.vvm", "8.vvm"];
    essential_models.iter().all(|model| models_dir.join(model).exists())
}

fn is_valid_dict_directory(dict_dir: &Path) -> bool {
    // Check for .dic files
    if let Ok(entries) = fs::read_dir(dict_dir) {
        entries.filter_map(|e| e.ok()).any(|e| {
            if let Some(file_name) = e.file_name().to_str() {
                file_name.ends_with(".dic")
            } else {
                false
            }
        })
    } else {
        false
    }
}
