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