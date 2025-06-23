use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
// use std::fs; // Removed unused import
use std::path::PathBuf;
use std::ptr;

#[cfg(feature = "dynamic_voicevox")]
use libloading::{Library, Symbol};

// Use bindgen-generated bindings if available
#[cfg(feature = "use_bindgen")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Define constants for result codes (bindgen may not expose them properly)
#[cfg(feature = "use_bindgen")]
pub const VOICEVOX_RESULT_OK: i32 = 0;

// If bindgen fails, provide manual bindings (simplified)
#[cfg(not(feature = "use_bindgen"))]
mod manual_bindings {
    // Dummy implementation for testing without actual VOICEVOX Core library
    #[cfg(not(feature = "link_voicevox"))]
    pub mod dummy_impl {
        use super::*;
        
        pub fn voicevox_make_default_load_onnxruntime_options() -> *const VoicevoxLoadOnnxruntimeOptions {
            std::ptr::null()
        }
        
        pub fn voicevox_onnxruntime_load_once(
            _options: *const VoicevoxLoadOnnxruntimeOptions,
            _onnxruntime: *mut *const VoicevoxOnnxruntime,
        ) -> c_int { 0 }
        
        pub fn voicevox_open_jtalk_rc_new(
            _open_jtalk_dic_dir: *const c_char,
            _open_jtalk_rc: *mut *mut OpenJtalkRc,
        ) -> c_int { 0 }
        
        pub fn voicevox_synthesizer_new(
            _onnxruntime: *const VoicevoxOnnxruntime,
            _open_jtalk_rc: *mut OpenJtalkRc,
            _options: VoicevoxInitializeOptions,
            _synthesizer: *mut *mut VoicevoxSynthesizer,
        ) -> c_int { 0 }
        
        pub fn voicevox_make_default_tts_options() -> *const VoicevoxTtsOptions {
            std::ptr::null()
        }
        
        pub fn voicevox_synthesizer_tts(
            _synthesizer: *mut VoicevoxSynthesizer,
            _text: *const c_char,
            _style_id: VoicevoxStyleId,
            _options: *const VoicevoxTtsOptions,
            wav_length: *mut uintptr_t,
            wav: *mut *mut c_uchar,
        ) -> c_int {
            // Return dummy WAV data for testing
            unsafe {
                *wav_length = 1024;
                *wav = libc::malloc(1024) as *mut c_uchar;
                std::ptr::write_bytes(*wav, 0, 1024);
            }
            0
        }
        
        pub fn voicevox_synthesizer_create_metas_json(
            _synthesizer: *mut VoicevoxSynthesizer,
        ) -> *mut c_char {
            let json = r#"[{"name":"TestSpeaker","styles":[{"name":"Normal","id":1}]}]"#;
            let c_str = std::ffi::CString::new(json).unwrap();
            c_str.into_raw()
        }
        
        pub fn voicevox_synthesizer_load_voice_model(
            _synthesizer: *const VoicevoxSynthesizer,
            _model: *const VoicevoxVoiceModelFile,
        ) -> c_int { 0 }
        
        pub fn voicevox_voice_model_file_open(
            _path: *const c_char,
            _model: *mut *mut VoicevoxVoiceModelFile,
        ) -> c_int { 0 }
        
        pub fn voicevox_voice_model_file_delete(_model: *mut VoicevoxVoiceModelFile) {}
        pub fn voicevox_synthesizer_delete(_synthesizer: *mut VoicevoxSynthesizer) {}
        pub fn voicevox_open_jtalk_rc_delete(_open_jtalk_rc: *mut OpenJtalkRc) {}
        pub fn voicevox_wav_free(_wav: *mut c_uchar) {}
        pub fn voicevox_json_free(_json: *mut c_char) {}
    }
    use libc::{c_char, c_int, c_uchar, c_uint, uintptr_t};

    pub const VOICEVOX_RESULT_OK: c_int = 0;
    pub type VoicevoxStyleId = c_uint;

    // Acceleration mode enum for macOS CPU-only processing
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub enum VoicevoxAccelerationMode {
        Auto = 0,
        Cpu = 1,
        Gpu = 2,
    }

    // Initialize options structure for CPU-only mode
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct VoicevoxInitializeOptions {
        pub acceleration_mode: VoicevoxAccelerationMode,
        pub cpu_num_threads: u16,
    }

    // Opaque types
    pub enum VoicevoxSynthesizer {}
    pub enum VoicevoxOnnxruntime {}
    pub enum OpenJtalkRc {}
    pub enum VoicevoxLoadOnnxruntimeOptions {}
    pub enum VoicevoxTtsOptions {}
    pub enum VoicevoxSynthesisOptions {}
    pub enum VoicevoxVoiceModelFile {}

    #[cfg(feature = "link_voicevox")]
    extern "C" {
        // Core initialization functions
        pub fn voicevox_make_default_load_onnxruntime_options(
        ) -> *const VoicevoxLoadOnnxruntimeOptions;
        pub fn voicevox_onnxruntime_load_once(
            options: *const VoicevoxLoadOnnxruntimeOptions,
            onnxruntime: *mut *const VoicevoxOnnxruntime,
        ) -> c_int;

        pub fn voicevox_open_jtalk_rc_new(
            open_jtalk_dic_dir: *const c_char,
            open_jtalk_rc: *mut *mut OpenJtalkRc,
        ) -> c_int;

        // Initialize options with CPU-only mode
        pub fn voicevox_synthesizer_new(
            onnxruntime: *const VoicevoxOnnxruntime,
            open_jtalk_rc: *mut OpenJtalkRc,
            options: VoicevoxInitializeOptions,
            synthesizer: *mut *mut VoicevoxSynthesizer,
        ) -> c_int;

        // TTS functions
        pub fn voicevox_make_default_tts_options() -> *const VoicevoxTtsOptions;
        pub fn voicevox_synthesizer_tts(
            synthesizer: *mut VoicevoxSynthesizer,
            text: *const c_char,
            style_id: VoicevoxStyleId,
            options: *const VoicevoxTtsOptions,
            wav_length: *mut uintptr_t,
            wav: *mut *mut c_uchar,
        ) -> c_int;

        // Metadata functions
        pub fn voicevox_synthesizer_create_metas_json(
            synthesizer: *mut VoicevoxSynthesizer,
        ) -> *mut c_char;

        // Model loading functions
        pub fn voicevox_synthesizer_load_voice_model(
            synthesizer: *const VoicevoxSynthesizer,
            model: *const VoicevoxVoiceModelFile,
        ) -> c_int;

        pub fn voicevox_voice_model_file_open(
            path: *const c_char,
            model: *mut *mut VoicevoxVoiceModelFile,
        ) -> c_int;

        pub fn voicevox_voice_model_file_delete(model: *mut VoicevoxVoiceModelFile);

        // Cleanup functions
        pub fn voicevox_wav_free(wav: *mut c_uchar);
        pub fn voicevox_json_free(json: *mut c_char);
        pub fn voicevox_synthesizer_delete(synthesizer: *mut VoicevoxSynthesizer);
        pub fn voicevox_open_jtalk_rc_delete(open_jtalk_rc: *mut OpenJtalkRc);
    }
    
    // Use dummy implementation when not linking to actual VOICEVOX Core
    #[cfg(not(any(feature = "link_voicevox", feature = "dynamic_voicevox")))]
    pub use dummy_impl::*;
}

#[cfg(not(feature = "use_bindgen"))]
pub use manual_bindings::*;

// Dynamic loading implementation for VOICEVOX Core
#[cfg(feature = "dynamic_voicevox")]
pub struct DynamicVoicevoxCore {
    _voicevox_lib: Library,
    _onnxruntime_lib: Library,
    
    // Function pointers
    pub voicevox_make_default_load_onnxruntime_options: Symbol<'static, unsafe extern "C" fn() -> *const VoicevoxLoadOnnxruntimeOptions>,
    pub voicevox_onnxruntime_load_once: Symbol<'static, unsafe extern "C" fn(*const VoicevoxLoadOnnxruntimeOptions, *mut *const VoicevoxOnnxruntime) -> libc::c_int>,
    pub voicevox_open_jtalk_rc_new: Symbol<'static, unsafe extern "C" fn(*const libc::c_char, *mut *mut OpenJtalkRc) -> libc::c_int>,
    pub voicevox_synthesizer_new: Symbol<'static, unsafe extern "C" fn(*const VoicevoxOnnxruntime, *mut OpenJtalkRc, VoicevoxInitializeOptions, *mut *mut VoicevoxSynthesizer) -> libc::c_int>,
    pub voicevox_make_default_tts_options: Symbol<'static, unsafe extern "C" fn() -> *const VoicevoxTtsOptions>,
    pub voicevox_synthesizer_tts: Symbol<'static, unsafe extern "C" fn(*mut VoicevoxSynthesizer, *const libc::c_char, VoicevoxStyleId, *const VoicevoxTtsOptions, *mut usize, *mut *mut libc::c_uchar) -> libc::c_int>,
    pub voicevox_synthesizer_create_metas_json: Symbol<'static, unsafe extern "C" fn(*mut VoicevoxSynthesizer) -> *mut libc::c_char>,
    pub voicevox_synthesizer_load_voice_model: Symbol<'static, unsafe extern "C" fn(*const VoicevoxSynthesizer, *const VoicevoxVoiceModelFile) -> libc::c_int>,
    pub voicevox_voice_model_file_open: Symbol<'static, unsafe extern "C" fn(*const libc::c_char, *mut *mut VoicevoxVoiceModelFile) -> libc::c_int>,
    pub voicevox_voice_model_file_delete: Symbol<'static, unsafe extern "C" fn(*mut VoicevoxVoiceModelFile)>,
    pub voicevox_synthesizer_delete: Symbol<'static, unsafe extern "C" fn(*mut VoicevoxSynthesizer)>,
    pub voicevox_open_jtalk_rc_delete: Symbol<'static, unsafe extern "C" fn(*mut OpenJtalkRc)>,
    pub voicevox_wav_free: Symbol<'static, unsafe extern "C" fn(*mut libc::c_uchar)>,
    pub voicevox_json_free: Symbol<'static, unsafe extern "C" fn(*mut libc::c_char)>,
}

#[cfg(feature = "dynamic_voicevox")]
impl DynamicVoicevoxCore {
    pub fn new() -> Result<Self> {
        let exe_dir = std::env::current_exe()
            .map_err(|e| anyhow!("Failed to get executable path: {}", e))?
            .parent()
            .ok_or_else(|| anyhow!("Failed to get executable directory"))?
            .to_path_buf();
        
        let current_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        
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
                println!("🔍 Trying to load VOICEVOX Core library: {}", path.display());
                unsafe { Library::new(path).ok() }
            })
            .ok_or_else(|| anyhow!("Failed to load VOICEVOX Core library from any path"))?;
        
        println!("✅ VOICEVOX Core library loaded successfully");
        
        // Load ONNX Runtime library
        let onnxruntime_lib = onnxruntime_lib_paths
            .iter()
            .find_map(|path| {
                println!("🔍 Trying to load ONNX Runtime library: {}", path.display());
                unsafe { Library::new(path).ok() }
            })
            .ok_or_else(|| anyhow!("Failed to load ONNX Runtime library from any path"))?;
        
        println!("✅ ONNX Runtime library loaded successfully");
        
        // Load function symbols
        let core = unsafe {
            DynamicVoicevoxCore {
                voicevox_make_default_load_onnxruntime_options: voicevox_lib.get(b"voicevox_make_default_load_onnxruntime_options\0")?,
                voicevox_onnxruntime_load_once: onnxruntime_lib.get(b"voicevox_onnxruntime_load_once\0")?,
                voicevox_open_jtalk_rc_new: voicevox_lib.get(b"voicevox_open_jtalk_rc_new\0")?,
                voicevox_synthesizer_new: voicevox_lib.get(b"voicevox_synthesizer_new\0")?,
                voicevox_make_default_tts_options: voicevox_lib.get(b"voicevox_make_default_tts_options\0")?,
                voicevox_synthesizer_tts: voicevox_lib.get(b"voicevox_synthesizer_tts\0")?,
                voicevox_synthesizer_create_metas_json: voicevox_lib.get(b"voicevox_synthesizer_create_metas_json\0")?,
                voicevox_synthesizer_load_voice_model: voicevox_lib.get(b"voicevox_synthesizer_load_voice_model\0")?,
                voicevox_voice_model_file_open: voicevox_lib.get(b"voicevox_voice_model_file_open\0")?,
                voicevox_voice_model_file_delete: voicevox_lib.get(b"voicevox_voice_model_file_delete\0")?,
                voicevox_synthesizer_delete: voicevox_lib.get(b"voicevox_synthesizer_delete\0")?,
                voicevox_open_jtalk_rc_delete: voicevox_lib.get(b"voicevox_open_jtalk_rc_delete\0")?,
                voicevox_wav_free: voicevox_lib.get(b"voicevox_wav_free\0")?,
                voicevox_json_free: voicevox_lib.get(b"voicevox_json_free\0")?,
                _voicevox_lib: voicevox_lib,
                _onnxruntime_lib: onnxruntime_lib,
            }
        };
        
        println!("✅ All VOICEVOX Core functions loaded successfully");
        Ok(core)
    }
}

// IPC Protocol Definitions
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DaemonRequest {
    Ping,
    Synthesize {
        text: String,
        style_id: u32,
        options: SynthesizeOptions,
    },
    ListSpeakers,
    LoadModel {
        model_name: String,
    },
    GetVoiceMapping,
    ResolveVoiceName {
        voice_name: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SynthesizeOptions {
    pub rate: f32,
    pub streaming: bool,
}

impl Default for SynthesizeOptions {
    fn default() -> Self {
        Self {
            rate: 1.0,
            streaming: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DaemonResponse {
    Pong,
    SynthesizeResult {
        wav_data: Vec<u8>,
    },
    SpeakersList {
        speakers: Vec<Speaker>,
    },
    VoiceMapping {
        mapping: HashMap<String, (u32, String)>,
    },
    VoiceResolution {
        style_id: u32,
        description: String,
    },
    Success,
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Speaker {
    pub name: String,
    #[serde(default)]
    pub speaker_uuid: String,
    pub styles: Vec<Style>,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Style {
    pub name: String,
    pub id: u32,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub style_type: Option<String>,
}

// VOICEVOX Core wrapper
pub struct VoicevoxCore {
    synthesizer: *mut VoicevoxSynthesizer,
    _open_jtalk_rc: *mut OpenJtalkRc,
    #[cfg(feature = "dynamic_voicevox")]
    _dynamic_core: Option<DynamicVoicevoxCore>,
}

unsafe impl Send for VoicevoxCore {}
unsafe impl Sync for VoicevoxCore {}

impl VoicevoxCore {
    pub fn new() -> Result<Self> {
        #[cfg(feature = "dynamic_voicevox")]
        {
            println!("🚀 Using dynamic VOICEVOX Core loading...");
            let dynamic_core = DynamicVoicevoxCore::new()?;
            Self::new_with_dynamic_core(dynamic_core)
        }
        #[cfg(all(feature = "link_voicevox", not(feature = "dynamic_voicevox")))]
        {
            println!("🚀 Using linked VOICEVOX Core...");
            Self::new_with_linked()
        }
        #[cfg(not(any(feature = "dynamic_voicevox", feature = "link_voicevox")))]
        {
            println!("🚀 Using dummy VOICEVOX Core implementation...");
            Self::new_with_dummy()
        }
    }
    
    #[cfg(feature = "dynamic_voicevox")]
    fn new_with_dynamic_core(dynamic_core: DynamicVoicevoxCore) -> Result<Self> {
        unsafe {
            // Load ONNX Runtime first
            let load_options = (dynamic_core.voicevox_make_default_load_onnxruntime_options)();
            let mut onnxruntime: *const VoicevoxOnnxruntime = ptr::null();

            let result = (dynamic_core.voicevox_onnxruntime_load_once)(load_options, &mut onnxruntime);
            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!(
                    "ONNX Runtime initialization failed: code {}",
                    result
                ));
            }

            // Initialize OpenJTalk
            let dict_path = find_openjtalk_dict()?;
            let dict_cstr = CString::new(dict_path)?;
            let mut open_jtalk_rc: *mut OpenJtalkRc = ptr::null_mut();

            let result = (dynamic_core.voicevox_open_jtalk_rc_new)(dict_cstr.as_ptr(), &mut open_jtalk_rc);
            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!(
                    "OpenJTalk RC initialization failed: code {}",
                    result
                ));
            }

            // Create synthesizer with CPU-only mode for macOS
            println!("🖥️  Initializing VOICEVOX Core in CPU-only mode...");

            // Create CPU-only initialization options structure
            let init_options = VoicevoxInitializeOptions {
                acceleration_mode: VoicevoxAccelerationMode::Cpu, // Force CPU mode, no GPU testing
                cpu_num_threads: 0, // Use default number of CPU threads (0 = auto-detect)
            };

            let mut synthesizer: *mut VoicevoxSynthesizer = ptr::null_mut();
            let result = (dynamic_core.voicevox_synthesizer_new)(
                onnxruntime,
                open_jtalk_rc,
                init_options,
                &mut synthesizer,
            );

            if result != VOICEVOX_RESULT_OK {
                (dynamic_core.voicevox_open_jtalk_rc_delete)(open_jtalk_rc);
                return Err(anyhow!("Synthesizer creation failed: code {}", result));
            }

            println!("✅ VOICEVOX Core initialization completed successfully");

            Ok(VoicevoxCore {
                synthesizer,
                _open_jtalk_rc: open_jtalk_rc,
                _dynamic_core: Some(dynamic_core),
            })
        }
    }
    
    #[cfg(feature = "link_voicevox")]
    fn new_with_linked() -> Result<Self> {
        unsafe {
            // Load ONNX Runtime first
            let load_options = voicevox_make_default_load_onnxruntime_options();
            let mut onnxruntime: *const VoicevoxOnnxruntime = ptr::null();

            let result = voicevox_onnxruntime_load_once(load_options, &mut onnxruntime);
            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!(
                    "ONNX Runtime initialization failed: code {}",
                    result
                ));
            }

            // Initialize OpenJTalk
            let dict_path = find_openjtalk_dict()?;
            let dict_cstr = CString::new(dict_path)?;
            let mut open_jtalk_rc: *mut OpenJtalkRc = ptr::null_mut();

            let result = voicevox_open_jtalk_rc_new(dict_cstr.as_ptr(), &mut open_jtalk_rc);
            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!(
                    "OpenJTalk RC initialization failed: code {}",
                    result
                ));
            }

            // Create synthesizer with CPU-only mode for macOS
            println!("🖥️  Initializing VOICEVOX Core in CPU-only mode...");

            // Create CPU-only initialization options structure
            let init_options = VoicevoxInitializeOptions {
                acceleration_mode: VoicevoxAccelerationMode::Cpu, // Force CPU mode, no GPU testing
                cpu_num_threads: 0, // Use default number of CPU threads (0 = auto-detect)
            };

            let mut synthesizer: *mut VoicevoxSynthesizer = ptr::null_mut();
            let result = voicevox_synthesizer_new(
                onnxruntime,
                open_jtalk_rc,
                init_options,
                &mut synthesizer,
            );

            if result != VOICEVOX_RESULT_OK {
                voicevox_open_jtalk_rc_delete(open_jtalk_rc);
                return Err(anyhow!("Synthesizer creation failed: code {}", result));
            }

            println!("✅ VOICEVOX Core initialization completed successfully");

            Ok(VoicevoxCore {
                synthesizer,
                _open_jtalk_rc: open_jtalk_rc,
                #[cfg(feature = "dynamic_voicevox")]
                _dynamic_core: None,
            })
        }
    }
    
    #[cfg(not(any(feature = "dynamic_voicevox", feature = "link_voicevox")))]
    fn new_with_dummy() -> Result<Self> {
        println!("📚 Found OpenJTalk dictionary: ./dict/open_jtalk_dic_utf_8-1.11");
        println!("🖥️  Initializing VOICEVOX Core in CPU-only mode...");
        println!("✅ VOICEVOX Core initialization completed successfully");
        
        Ok(VoicevoxCore {
            synthesizer: ptr::null_mut(),
            _open_jtalk_rc: ptr::null_mut(),
            #[cfg(feature = "dynamic_voicevox")]
            _dynamic_core: None,
        })
    }

    pub fn load_all_models(&self) -> Result<()> {
        // Find the models directory
        let models_dir = find_models_dir()?;

        println!("📦 Loading all VVM models from: {}", models_dir.display());

        // Load all VVM files
        let mut loaded_count = 0;
        if let Ok(entries) = std::fs::read_dir(&models_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Some(file_name) = entry.file_name().to_str() {
                    if file_name.ends_with(".vvm") {
                        let model_path = entry.path();
                        if let Some(path_str) = model_path.to_str() {
                            if let Ok(path_cstr) = CString::new(path_str) {
                                unsafe {
                                    let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                                    let result = voicevox_voice_model_file_open(
                                        path_cstr.as_ptr(),
                                        &mut model,
                                    );
                                    if result == VOICEVOX_RESULT_OK {
                                        let load_result = voicevox_synthesizer_load_voice_model(
                                            self.synthesizer,
                                            model,
                                        );
                                        if load_result == VOICEVOX_RESULT_OK {
                                            loaded_count += 1;
                                            println!("  ✅ Loaded: {}", file_name);
                                        } else if load_result == 18 {
                                            // MODEL_ALREADY_LOADED_ERROR
                                            println!("  ℹ️  Model {} already loaded", file_name);
                                            loaded_count += 1;
                                        } else {
                                            println!(
                                                "  ⚠️  Failed to load: {} (code: {})",
                                                file_name, load_result
                                            );
                                        }
                                        voicevox_voice_model_file_delete(model);
                                    } else {
                                        println!(
                                            "  ⚠️  Failed to open: {} (code: {})",
                                            file_name, result
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if loaded_count > 0 {
            println!("✅ Successfully loaded {} VVM models", loaded_count);
        } else {
            println!("⚠️  No VVM models were loaded");
        }

        Ok(())
    }

    pub fn load_minimal_models(&self) -> Result<()> {
        // Load only essential models for faster startup (minimal mode)
        // Priority: ずんだもん (3.vvm), 四国めたん (2.vvm), 春日部つむぎ (8.vvm)
        let default_models = ["3.vvm", "2.vvm", "8.vvm"];

        let models_dir = find_models_dir()?;

        println!("📦 Loading minimal VVM models for faster startup...");

        let mut loaded_count = 0;
        for model_name in &default_models {
            let model_path = models_dir.join(model_name);
            if model_path.exists() {
                if let Some(path_str) = model_path.to_str() {
                    if let Ok(path_cstr) = CString::new(path_str) {
                        unsafe {
                            let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                            let result =
                                voicevox_voice_model_file_open(path_cstr.as_ptr(), &mut model);
                            if result == VOICEVOX_RESULT_OK {
                                let load_result =
                                    voicevox_synthesizer_load_voice_model(self.synthesizer, model);
                                if load_result == VOICEVOX_RESULT_OK {
                                    loaded_count += 1;
                                    println!("  ✅ Loaded: {}", model_name);
                                } else if load_result == 18 {
                                    // MODEL_ALREADY_LOADED_ERROR
                                    println!("  ℹ️  Model {} already loaded", model_name);
                                    loaded_count += 1;
                                } else {
                                    println!(
                                        "  ⚠️  Failed to load: {} (code: {})",
                                        model_name, load_result
                                    );
                                }
                                voicevox_voice_model_file_delete(model);
                            } else {
                                println!("  ⚠️  Failed to open: {} (code: {})", model_name, result);
                            }
                        }
                    }
                }
            } else {
                println!("  ⚠️  Model not found: {}", model_name);
            }
        }

        if loaded_count > 0 {
            println!("✅ Successfully loaded {} minimal VVM models", loaded_count);
        } else {
            println!("⚠️  No minimal VVM models were loaded");
        }

        Ok(())
    }

    pub fn load_specific_model(&self, model_name: &str) -> Result<()> {
        let models_dir = find_models_dir()?;
        let model_path = models_dir.join(format!("{}.vvm", model_name));

        if !model_path.exists() {
            return Err(anyhow!("Model not found: {}.vvm", model_name));
        }

        println!("📦 Loading model: {}.vvm", model_name);

        if let Some(path_str) = model_path.to_str() {
            if let Ok(path_cstr) = CString::new(path_str) {
                unsafe {
                    let mut model: *mut VoicevoxVoiceModelFile = ptr::null_mut();
                    let result = voicevox_voice_model_file_open(path_cstr.as_ptr(), &mut model);
                    if result == VOICEVOX_RESULT_OK {
                        let load_result =
                            voicevox_synthesizer_load_voice_model(self.synthesizer, model);
                        if load_result == VOICEVOX_RESULT_OK {
                            println!("  ✅ Successfully loaded: {}.vvm", model_name);
                        } else if load_result == 18 {
                            // MODEL_ALREADY_LOADED_ERROR
                            println!("  ℹ️  Model {}.vvm already loaded", model_name);
                        } else {
                            voicevox_voice_model_file_delete(model);
                            return Err(anyhow!(
                                "Failed to load model: {} (code: {})",
                                model_name,
                                load_result
                            ));
                        }
                        voicevox_voice_model_file_delete(model);
                    } else {
                        return Err(anyhow!(
                            "Failed to open model: {} (code: {})",
                            model_name,
                            result
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn synthesize(&self, text: &str, style_id: u32) -> Result<Vec<u8>> {
        #[cfg(any(feature = "link_voicevox", feature = "dynamic_voicevox"))]
        {
            self.synthesize_real(text, style_id)
        }
        #[cfg(not(any(feature = "link_voicevox", feature = "dynamic_voicevox")))]
        {
            self.synthesize_dummy(text, style_id)
        }
    }
    
    #[cfg(any(feature = "link_voicevox", feature = "dynamic_voicevox"))]
    fn synthesize_real(&self, text: &str, style_id: u32) -> Result<Vec<u8>> {
        unsafe {
            let text_cstr = CString::new(text)?;
            let tts_options = voicevox_make_default_tts_options();
            let mut wav_data: *mut u8 = ptr::null_mut();
            let mut wav_length: usize = 0;

            let result = voicevox_synthesizer_tts(
                self.synthesizer,
                text_cstr.as_ptr(),
                style_id,
                tts_options,
                &mut wav_length,
                &mut wav_data,
            );

            if result != VOICEVOX_RESULT_OK {
                return Err(anyhow!("Speech synthesis failed: code {}", result));
            }

            if wav_data.is_null() || wav_length == 0 {
                return Err(anyhow!("Audio data was not generated"));
            }

            let wav_vec = std::slice::from_raw_parts(wav_data, wav_length).to_vec();
            voicevox_wav_free(wav_data);
            Ok(wav_vec)
        }
    }
    
    #[cfg(not(any(feature = "link_voicevox", feature = "dynamic_voicevox")))]
    fn synthesize_dummy(&self, _text: &str, _style_id: u32) -> Result<Vec<u8>> {
        // Return 1024 bytes of dummy WAV data
        Ok(vec![0; 1024])
    }

    pub fn get_speakers(&self) -> Result<Vec<Speaker>> {
        #[cfg(any(feature = "link_voicevox", feature = "dynamic_voicevox"))]
        {
            self.get_speakers_real()
        }
        #[cfg(not(any(feature = "link_voicevox", feature = "dynamic_voicevox")))]
        {
            self.get_speakers_dummy()
        }
    }
    
    #[cfg(any(feature = "link_voicevox", feature = "dynamic_voicevox"))]
    fn get_speakers_real(&self) -> Result<Vec<Speaker>> {
        unsafe {
            let metas_json = voicevox_synthesizer_create_metas_json(self.synthesizer);
            if metas_json.is_null() {
                return Err(anyhow!("Failed to get speaker metadata"));
            }

            let metas_str = CStr::from_ptr(metas_json).to_str()?;
            let speakers: Vec<Speaker> = serde_json::from_str(metas_str)
                .map_err(|e| anyhow!("Failed to parse speaker metadata: {}", e))?;

            voicevox_json_free(metas_json);
            Ok(speakers)
        }
    }
    
    #[cfg(not(any(feature = "link_voicevox", feature = "dynamic_voicevox")))]
    fn get_speakers_dummy(&self) -> Result<Vec<Speaker>> {
        // Return dummy speaker data
        Ok(vec![Speaker {
            name: "TestSpeaker".to_string(),
            styles: vec![Style {
                name: "Normal".to_string(),
                id: 1,
                style_type: None,
            }],
            version: "".to_string(),
        }])
    }
}

impl Drop for VoicevoxCore {
    fn drop(&mut self) {
        #[cfg(any(feature = "link_voicevox", feature = "dynamic_voicevox"))]
        {
            unsafe {
                if !self.synthesizer.is_null() {
                    voicevox_synthesizer_delete(self.synthesizer);
                }
                if !self._open_jtalk_rc.is_null() {
                    voicevox_open_jtalk_rc_delete(self._open_jtalk_rc);
                }
            }
        }
    }
}

// Utility functions

// Helper function to find VVM models directory
pub fn find_models_dir() -> Result<PathBuf> {
    let mut search_paths = Vec::new();
    
    // Priority 1: Package installation path (when used as a Nix package)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(pkg_root) = exe_path.parent().and_then(|p| p.parent()) {
            search_paths.push(pkg_root.join("share/voicevox/models"));
        }
    }
    
    let mut additional_paths = vec![
        
        // Priority 2: Local models directory (current working dir)
        Some(PathBuf::from("./models")),
        
        // Priority 3: Home directory models
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".voicevox/models")),
        
        // Priority 4: XDG data directory
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(|xdg| PathBuf::from(xdg).join("voicevox/models"))
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".local/share/voicevox/models"))
            }),
        
        // Priority 5: System directories
        Some(PathBuf::from("/usr/local/share/voicevox/models")),
        Some(PathBuf::from("/usr/share/voicevox/models")),
        Some(PathBuf::from("/opt/voicevox/models")),
        
        // Priority 6: macOS specific paths
        Some(PathBuf::from("/Applications/VOICEVOX.app/Contents/Resources/models")),
        Some(PathBuf::from("/opt/homebrew/share/voicevox/models")),
        
        // Priority 7: Development/workspace paths (generic search)
        std::env::current_dir()
            .ok()
            .and_then(|current_dir| {
                current_dir
                    .ancestors()
                    .find(|a| a.join("models").exists())
                    .map(|p| p.join("models"))
            }),
        
        // Priority 8: Environment variable (explicit override)
        std::env::var("VOICEVOX_MODELS_DIR")
            .ok()
            .map(PathBuf::from),
    ];
    
    search_paths.extend(additional_paths.into_iter().flatten());

    for path_option in search_paths.into_iter() {
        if path_option.exists() && is_valid_models_directory(&path_option) {
            println!("📁 Found models directory: {}", path_option.display());
            return Ok(path_option);
        }
    }
    
    Err(anyhow!("VVM models directory not found. Please ensure models are installed in one of the standard locations or set VOICEVOX_MODELS_DIR environment variable."))
}

// Helper function to validate models directory
fn is_valid_models_directory(path: &PathBuf) -> bool {
    if let Ok(entries) = std::fs::read_dir(path) {
        entries.filter_map(|e| e.ok()).any(|e| {
            if let Some(file_name) = e.file_name().to_str() {
                file_name.ends_with(".vvm")
            } else {
                false
            }
        })
    } else {
        false
    }
}

// Helper function to check if a directory contains .dic files
fn has_dic_files(dict_path: &PathBuf) -> bool {
    if let Ok(entries) = std::fs::read_dir(dict_path) {
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

pub fn find_openjtalk_dict() -> Result<String> {
    let mut search_paths = Vec::new();
    
    // Priority 1: Package installation path (when used as a Nix package)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(pkg_root) = exe_path.parent().and_then(|p| p.parent()) {
            search_paths.push(pkg_root.join("share/voicevox/dict"));
        }
    }
    
    let mut additional_paths = vec![
        
        // Priority 2: Local dictionary (current working dir)
        Some(PathBuf::from("./dict")),
        Some(PathBuf::from("./dict/open_jtalk_dic_utf_8-1.11")),
        
        // Priority 3: Home directory dictionary
        std::env::var("HOME")
            .ok()
            .map(|home| PathBuf::from(home).join(".voicevox/dict")),
        
        // Priority 4: XDG data directory
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(|xdg| PathBuf::from(xdg).join("voicevox/dict"))
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".local/share/voicevox/dict"))
            }),
        
        // Priority 5: System OpenJTalk paths
        Some(PathBuf::from("/usr/local/share/open-jtalk/dic")),
        Some(PathBuf::from("/usr/share/open-jtalk/dic")),
        Some(PathBuf::from("/opt/open-jtalk/dic")),
        
        // Priority 6: System VOICEVOX paths
        Some(PathBuf::from("/usr/local/share/voicevox/dict")),
        Some(PathBuf::from("/usr/share/voicevox/dict")),
        Some(PathBuf::from("/opt/voicevox/dict")),
        
        // Priority 7: macOS specific paths
        Some(PathBuf::from("/Applications/VOICEVOX.app/Contents/Resources/dict")),
        Some(PathBuf::from("/opt/homebrew/share/open-jtalk/dic")),
        Some(PathBuf::from("/opt/homebrew/share/voicevox/dict")),
        Some(PathBuf::from("/opt/local/share/open-jtalk/dic")),
        
        // Priority 8: Development/workspace paths (generic search)
        std::env::current_dir()
            .ok()
            .and_then(|current_dir| {
                current_dir
                    .ancestors()
                    .find(|a| a.join("dict").exists())
                    .map(|p| p.join("dict"))
            }),
        
        // Priority 9: Environment variable (explicit override)
        std::env::var("VOICEVOX_DICT_DIR")
            .ok()
            .map(PathBuf::from),
        std::env::var("OPENJTALK_DICT_DIR")
            .ok()
            .map(PathBuf::from),
    ];
    
    search_paths.extend(additional_paths.into_iter().flatten());

    for path_option in search_paths.into_iter() {
        if path_option.exists() && has_dic_files(&path_option) {
            let path_str = path_option.to_string_lossy().to_string();
            println!("📚 Found OpenJTalk dictionary: {}", path_str);
            return Ok(path_str);
        }
    }
    
    Err(anyhow!("OpenJTalk dictionary not found. Please ensure the dictionary is installed in one of the standard locations or set VOICEVOX_DICT_DIR/OPENJTALK_DICT_DIR environment variable."))
}

// 音声IDから必要なVVMモデル番号を取得
pub fn get_model_for_voice_id(voice_id: u32) -> Option<u32> {
    match voice_id {
        // ずんだもん (3.vvm)
        1 | 3 | 7 | 5 | 22 | 38 => Some(3),
        // 四国めたん (2.vvm)
        2 | 0 | 6 | 4 | 36 | 37 => Some(2),
        // 春日部つむぎ (8.vvm)
        8 | 83 | 84 => Some(8),
        // 雨晴はう (10.vvm)
        10 | 85 => Some(10),
        // 波音リツ (9.vvm)
        9 | 65 => Some(9),
        // 玄野武宏 (11.vvm)
        11 | 39 | 40 | 41 => Some(11),
        // 白上虎太郎 (12.vvm)
        12 | 32 | 33 => Some(12),
        // 青山龍星 (13.vvm)
        13 | 86 | 87 | 88 | 89 | 90 => Some(13),
        // 冥鳴ひまり (14.vvm)
        14 => Some(14),
        // 九州そら (16.vvm)
        15 | 16 | 17 | 18 | 19 => Some(16),
        // もち子さん (17.vvm)
        20 => Some(17),
        // 剣崎雌雄 (18.vvm)
        21 => Some(18),
        // デフォルトは不明
        _ => None,
    }
}

// 音声名からスタイルIDへのマッピング
pub fn get_voice_mapping() -> HashMap<&'static str, (u32, &'static str)> {
    let mut voices = HashMap::new();

    // ずんだもん（全モード）
    voices.insert("zundamon", (3, "ずんだもん (ノーマル)"));
    voices.insert("zundamon-normal", (3, "ずんだもん (ノーマル)"));
    voices.insert("zundamon-amama", (1, "ずんだもん (あまあま)"));
    voices.insert("zundamon-tsundere", (7, "ずんだもん (ツンツン)"));
    voices.insert("zundamon-sexy", (5, "ずんだもん (セクシー)"));
    voices.insert("zundamon-whisper", (22, "ずんだもん (ささやき)"));
    voices.insert("zundamon-excited", (38, "ずんだもん (ヘロヘロ)"));

    // 四国めたん（全モード）
    voices.insert("metan", (2, "四国めたん (ノーマル)"));
    voices.insert("metan-normal", (2, "四国めたん (ノーマル)"));
    voices.insert("metan-amama", (0, "四国めたん (あまあま)"));
    voices.insert("metan-tsundere", (6, "四国めたん (ツンツン)"));
    voices.insert("metan-sexy", (4, "四国めたん (セクシー)"));
    voices.insert("metan-whisper", (36, "四国めたん (ささやき)"));
    voices.insert("metan-excited", (37, "四国めたん (ヘロヘロ)"));

    // 春日部つむぎ
    voices.insert("tsumugi", (8, "春日部つむぎ (ノーマル)"));
    voices.insert("tsumugi-normal", (8, "春日部つむぎ (ノーマル)"));

    // 雨晴はう
    voices.insert("hau", (10, "雨晴はう (ノーマル)"));
    voices.insert("hau-normal", (10, "雨晴はう (ノーマル)"));

    // 波音リツ
    voices.insert("ritsu", (9, "波音リツ (ノーマル)"));
    voices.insert("ritsu-normal", (9, "波音リツ (ノーマル)"));

    // 玄野武宏
    voices.insert("takehiro", (11, "玄野武宏 (ノーマル)"));
    voices.insert("takehiro-normal", (11, "玄野武宏 (ノーマル)"));
    voices.insert("takehiro-excited", (39, "玄野武宏 (喜び)"));
    voices.insert("takehiro-tsundere", (40, "玄野武宏 (ツンギレ)"));
    voices.insert("takehiro-sad", (41, "玄野武宏 (悲しみ)"));

    // 白上虎太郎
    voices.insert("kohtaro", (12, "白上虎太郎 (ふつう)"));
    voices.insert("kohtaro-normal", (12, "白上虎太郎 (ふつう)"));
    voices.insert("kohtaro-excited", (32, "白上虎太郎 (わーい)"));
    voices.insert("kohtaro-angry", (33, "白上虎太郎 (びくびく)"));

    // 青山龍星
    voices.insert("ryusei", (13, "青山龍星 (ノーマル)"));
    voices.insert("ryusei-normal", (13, "青山龍星 (ノーマル)"));
    voices.insert("ryusei-excited", (86, "青山龍星 (熱血)"));
    voices.insert("ryusei-cool", (87, "青山龍星 (不機嫌)"));
    voices.insert("ryusei-sad", (88, "青山龍星 (喜び)"));
    voices.insert("ryusei-surprised", (89, "青山龍星 (しっとり)"));
    voices.insert("ryusei-whisper", (90, "青山龍星 (かなしみ)"));

    // 冥鳴ひまり
    voices.insert("himari", (14, "冥鳴ひまり (ノーマル)"));
    voices.insert("himari-normal", (14, "冥鳴ひまり (ノーマル)"));

    // 九州そら
    voices.insert("sora", (16, "九州そら (ノーマル)"));
    voices.insert("sora-normal", (16, "九州そら (ノーマル)"));
    voices.insert("sora-amama", (15, "九州そら (あまあま)"));
    voices.insert("sora-tsundere", (18, "九州そら (ツンツン)"));
    voices.insert("sora-sexy", (17, "九州そら (セクシー)"));
    voices.insert("sora-whisper", (19, "九州そら (ささやき)"));

    // もち子さん
    voices.insert("mochiko", (20, "もち子さん (ノーマル)"));
    voices.insert("mochiko-normal", (20, "もち子さん (ノーマル)"));

    // 剣崎雌雄
    voices.insert("menou", (21, "剣崎雌雄 (ノーマル)"));
    voices.insert("menou-normal", (21, "剣崎雌雄 (ノーマル)"));

    // デフォルトエイリアス
    voices.insert("default", (3, "ずんだもん (ノーマル)"));

    voices
}

pub fn resolve_voice_name(voice_name: &str) -> Result<(u32, String)> {
    let voices = get_voice_mapping();

    // 音声一覧表示の特別なケース
    if voice_name == "?" {
        println!("🎭 Available VOICEVOX voices:");
        println!();

        // キャラクター別にグループ化して表示
        println!("  📝 ずんだもん:");
        println!("    zundamon, zundamon-normal    (ID: 3)  - ずんだもん (ノーマル)");
        println!("    zundamon-amama              (ID: 1)  - ずんだもん (あまあま)");
        println!("    zundamon-tsundere           (ID: 7)  - ずんだもん (ツンツン)");
        println!("    zundamon-sexy               (ID: 5)  - ずんだもん (セクシー)");
        println!("    zundamon-whisper            (ID: 22) - ずんだもん (ささやき)");
        println!("    zundamon-excited            (ID: 38) - ずんだもん (ヘロヘロ)");
        println!();

        println!("  🍊 四国めたん:");
        println!("    metan, metan-normal         (ID: 2)  - 四国めたん (ノーマル)");
        println!("    metan-amama                 (ID: 0)  - 四国めたん (あまあま)");
        println!("    metan-tsundere              (ID: 6)  - 四国めたん (ツンツン)");
        println!("    metan-sexy                  (ID: 4)  - 四国めたん (セクシー)");
        println!("    metan-whisper               (ID: 36) - 四国めたん (ささやき)");
        println!("    metan-excited               (ID: 37) - 四国めたん (ヘロヘロ)");
        println!();

        println!("  🌸 その他のキャラクター:");
        println!("    tsumugi                     (ID: 8)  - 春日部つむぎ (ノーマル)");
        println!("    hau                         (ID: 10) - 雨晴はう (ノーマル)");
        println!("    ritsu                       (ID: 9)  - 波音リツ (ノーマル)");
        println!("    takehiro                    (ID: 11) - 玄野武宏 (ノーマル)");
        println!("    kohtaro                     (ID: 12) - 白上虎太郎 (ふつう)");
        println!("    ryusei                      (ID: 13) - 青山龍星 (ノーマル)");
        println!("    sora                        (ID: 16) - 九州そら (ノーマル)");
        println!();

        println!("  💡 Tips:");
        println!("    - 数値IDを直接指定することも可能です: -v 3");
        println!("    - キャラクター名のみでデフォルトモードを使用: -v zundamon");
        println!("    - 特定のモードを指定: -v zundamon-amama");
        println!();

        std::process::exit(0);
    }

    // 直接的な数値指定をサポート
    if let Ok(style_id) = voice_name.parse::<u32>() {
        return Ok((style_id, format!("Style ID {}", style_id)));
    }

    // 音声名から検索
    if let Some((style_id, description)) = voices.get(voice_name) {
        Ok((*style_id, description.to_string()))
    } else {
        Err(anyhow!(
            "Unknown voice: '{}'. Use -v ? to list available voices.",
            voice_name
        ))
    }
}

// Socket path for IPC
pub fn get_socket_path() -> PathBuf {
    // Priority 1: XDG runtime directory (Linux standard)
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("voicevox-daemon.sock");
    }
    
    // Priority 2: User's home directory
    if let Ok(home_dir) = std::env::var("HOME") {
        let user_socket = PathBuf::from(home_dir).join(".voicevox/daemon.sock");
        // Create directory if it doesn't exist
        if let Some(parent) = user_socket.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return user_socket;
    }
    
    // Priority 3: System temp directory with user-specific name
    if let Ok(temp_dir) = std::env::var("TMPDIR") {
        let user_id = std::process::id();
        return PathBuf::from(temp_dir).join(format!("voicevox-daemon-{}.sock", user_id));
    }
    
    // Priority 4: Platform-specific fallback
    let user_id = std::process::id();
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/tmp").join(format!("voicevox-daemon-{}.sock", user_id))
    }
    #[cfg(not(target_os = "macos"))]
    {
        PathBuf::from("/tmp/voicevox-daemon.sock")
    }
}