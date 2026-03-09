use crate::interface::AppOutput;

const HELP_TEXT: &str = r#"Available VOICEVOX voices:

  Use one of these options to discover voices:
    --list-models        - Show available VVM models
    --list-speakers      - Show all speaker details from loaded models
    --speaker-id N       - Use specific style ID directly
    --model N            - Use model N.vvm

  Examples:
    voicevox-say --speaker-id 3 \"text\"
    voicevox-say --model 3 \"text\"
"#;

pub fn print_voice_help(output: &dyn AppOutput) {
    output.info(HELP_TEXT);
}
