use anyhow::{anyhow, Result};
use libloading::{Library, Symbol};
use std::path::PathBuf;

use super::manual::*;

// Dynamic loading implementation for VOICEVOX Core
pub struct DynamicVoicevoxCore {
    _voicevox_lib: Library,
    _onnxruntime_lib: Library,

    // Function pointers
    pub voicevox_make_default_load_onnxruntime_options:
        Symbol<'static, unsafe extern "C" fn() -> *const VoicevoxLoadOnnxruntimeOptions>,
    pub voicevox_onnxruntime_load_once: Symbol<
        'static,
        unsafe extern "C" fn(
            *const VoicevoxLoadOnnxruntimeOptions,
            *mut *const VoicevoxOnnxruntime,
        ) -> libc::c_int,
    >,
    pub voicevox_open_jtalk_rc_new: Symbol<
        'static,
        unsafe extern "C" fn(*const libc::c_char, *mut *mut OpenJtalkRc) -> libc::c_int,
    >,
    pub voicevox_synthesizer_new: Symbol<
        'static,
        unsafe extern "C" fn(
            *const VoicevoxOnnxruntime,
            *mut OpenJtalkRc,
            VoicevoxInitializeOptions,
            *mut *mut VoicevoxSynthesizer,
        ) -> libc::c_int,
    >,
    pub voicevox_make_default_tts_options:
        Symbol<'static, unsafe extern "C" fn() -> *const VoicevoxTtsOptions>,
    pub voicevox_synthesizer_tts: Symbol<
        'static,
        unsafe extern "C" fn(
            *mut VoicevoxSynthesizer,
            *const libc::c_char,
            VoicevoxStyleId,
            *const VoicevoxTtsOptions,
            *mut usize,
            *mut *mut libc::c_uchar,
        ) -> libc::c_int,
    >,
    pub voicevox_synthesizer_create_metas_json:
        Symbol<'static, unsafe extern "C" fn(*mut VoicevoxSynthesizer) -> *mut libc::c_char>,
    pub voicevox_synthesizer_load_voice_model: Symbol<
        'static,
        unsafe extern "C" fn(
            *const VoicevoxSynthesizer,
            *const VoicevoxVoiceModelFile,
        ) -> libc::c_int,
    >,
    pub voicevox_voice_model_file_open: Symbol<
        'static,
        unsafe extern "C" fn(*const libc::c_char, *mut *mut VoicevoxVoiceModelFile) -> libc::c_int,
    >,
    pub voicevox_voice_model_file_delete:
        Symbol<'static, unsafe extern "C" fn(*mut VoicevoxVoiceModelFile)>,
    pub voicevox_synthesizer_delete:
        Symbol<'static, unsafe extern "C" fn(*mut VoicevoxSynthesizer)>,
    pub voicevox_open_jtalk_rc_delete: Symbol<'static, unsafe extern "C" fn(*mut OpenJtalkRc)>,
    pub voicevox_wav_free: Symbol<'static, unsafe extern "C" fn(*mut libc::c_uchar)>,
    pub voicevox_json_free: Symbol<'static, unsafe extern "C" fn(*mut libc::c_char)>,
}

impl DynamicVoicevoxCore {
    pub fn new() -> Result<Self> {
        let exe_dir = std::env::current_exe()
            .map_err(|e| anyhow!("Failed to get executable path: {}", e))?
            .parent()
            .ok_or_else(|| anyhow!("Failed to get executable directory"))?
            .to_path_buf();

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Check for Nix store environment
        let is_nix_store = exe_dir.starts_with("/nix/store");

        let mut voicevox_lib_paths = Vec::new();
        let mut onnxruntime_lib_paths = Vec::new();

        // Priority 1: Nix store paths (when running from /nix/store)
        if is_nix_store {
            // Look for VOICEVOX Core in Nix store structure
            if let Some(nix_path) = find_nix_voicevox_path(&exe_dir) {
                voicevox_lib_paths.push(nix_path.join("lib/libvoicevox_core.dylib"));
                onnxruntime_lib_paths.push(nix_path.join("lib/libvoicevox_onnxruntime.dylib"));
            }
        }

        // Priority 2: Current directory relative paths
        voicevox_lib_paths.extend([
            current_dir.join("voicevox_core/c_api/lib/libvoicevox_core.dylib"),
            PathBuf::from("./voicevox_core/c_api/lib/libvoicevox_core.dylib"),
        ]);
        onnxruntime_lib_paths.extend([
            current_dir.join("voicevox_core/onnxruntime/lib/libvoicevox_onnxruntime.dylib"),
            PathBuf::from("./voicevox_core/onnxruntime/lib/libvoicevox_onnxruntime.dylib"),
        ]);

        // Priority 3: Executable directory relative paths
        voicevox_lib_paths.extend([
            exe_dir.join("../voicevox_core/c_api/lib/libvoicevox_core.dylib"),
            exe_dir.join("voicevox_core/c_api/lib/libvoicevox_core.dylib"),
            exe_dir.join("lib/libvoicevox_core.dylib"),
        ]);
        onnxruntime_lib_paths.extend([
            exe_dir.join("../voicevox_core/onnxruntime/lib/libvoicevox_onnxruntime.dylib"),
            exe_dir.join("voicevox_core/onnxruntime/lib/libvoicevox_onnxruntime.dylib"),
            exe_dir.join("lib/libvoicevox_onnxruntime.dylib"),
        ]);

        // Priority 4: System paths
        voicevox_lib_paths.extend([
            PathBuf::from("/usr/local/lib/libvoicevox_core.dylib"),
            PathBuf::from("/opt/homebrew/lib/libvoicevox_core.dylib"),
        ]);
        onnxruntime_lib_paths.extend([
            PathBuf::from("/usr/local/lib/libvoicevox_onnxruntime.dylib"),
            PathBuf::from("/opt/homebrew/lib/libvoicevox_onnxruntime.dylib"),
        ]);

        // Load VOICEVOX Core library
        let voicevox_lib = voicevox_lib_paths
            .iter()
            .find_map(|path| {
                println!("Trying to load VOICEVOX Core library: {}", path.display());
                unsafe { Library::new(path).ok() }
            })
            .ok_or_else(|| anyhow!("Failed to load VOICEVOX Core library from any path"))?;

        println!("VOICEVOX Core library loaded successfully");

        // Load ONNX Runtime library
        let onnxruntime_lib = onnxruntime_lib_paths
            .iter()
            .find_map(|path| {
                println!("Trying to load ONNX Runtime library: {}", path.display());
                unsafe { Library::new(path).ok() }
            })
            .ok_or_else(|| anyhow!("Failed to load ONNX Runtime library from any path"))?;

        println!("ONNX Runtime library loaded successfully");

        // Load function symbols
        let core = unsafe {
            DynamicVoicevoxCore {
                voicevox_make_default_load_onnxruntime_options: voicevox_lib
                    .get(b"voicevox_make_default_load_onnxruntime_options\0")?,
                voicevox_onnxruntime_load_once: onnxruntime_lib
                    .get(b"voicevox_onnxruntime_load_once\0")?,
                voicevox_open_jtalk_rc_new: voicevox_lib.get(b"voicevox_open_jtalk_rc_new\0")?,
                voicevox_synthesizer_new: voicevox_lib.get(b"voicevox_synthesizer_new\0")?,
                voicevox_make_default_tts_options: voicevox_lib
                    .get(b"voicevox_make_default_tts_options\0")?,
                voicevox_synthesizer_tts: voicevox_lib.get(b"voicevox_synthesizer_tts\0")?,
                voicevox_synthesizer_create_metas_json: voicevox_lib
                    .get(b"voicevox_synthesizer_create_metas_json\0")?,
                voicevox_synthesizer_load_voice_model: voicevox_lib
                    .get(b"voicevox_synthesizer_load_voice_model\0")?,
                voicevox_voice_model_file_open: voicevox_lib
                    .get(b"voicevox_voice_model_file_open\0")?,
                voicevox_voice_model_file_delete: voicevox_lib
                    .get(b"voicevox_voice_model_file_delete\0")?,
                voicevox_synthesizer_delete: voicevox_lib.get(b"voicevox_synthesizer_delete\0")?,
                voicevox_open_jtalk_rc_delete: voicevox_lib
                    .get(b"voicevox_open_jtalk_rc_delete\0")?,
                voicevox_wav_free: voicevox_lib.get(b"voicevox_wav_free\0")?,
                voicevox_json_free: voicevox_lib.get(b"voicevox_json_free\0")?,
                _voicevox_lib: voicevox_lib,
                _onnxruntime_lib: onnxruntime_lib,
            }
        };

        println!("All VOICEVOX Core functions loaded successfully");
        Ok(core)
    }
}

fn find_nix_voicevox_path(exe_dir: &PathBuf) -> Option<PathBuf> {
    // Simplified Nix store path discovery
    exe_dir.ancestors().find_map(|p| {
        let voicevox_path = p.join("lib");
        if voicevox_path.join("libvoicevox_core.dylib").exists() {
            Some(p.to_path_buf())
        } else {
            None
        }
    })
}