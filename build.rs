use std::env;

fn main() {
    // Embed OpenJTalk dictionary path at compile time
    // This is the ONLY environment variable needed for OpenJTalk configuration
    // The path will be embedded in the binary as VOICEVOX_OPENJTALK_DICT_EMBEDDED
    if let Ok(dict_path) = env::var("OPENJTALK_DICT_PATH") {
        println!("cargo:rustc-env=VOICEVOX_OPENJTALK_DICT_EMBEDDED={dict_path}");
    }

    // Rerun build if the environment variable changes
    println!("cargo:rerun-if-env-changed=OPENJTALK_DICT_PATH");
}
