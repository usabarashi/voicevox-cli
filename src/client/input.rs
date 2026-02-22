use anyhow::Result;
use std::fs;
use std::io::{self, Read};

fn read_stdin_trimmed() -> Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim_end().to_string())
}

fn read_input_file(file_path: &str) -> Result<String> {
    if file_path == "-" {
        read_stdin_trimmed()
    } else {
        fs::read_to_string(file_path).map_err(Into::into)
    }
}

/// Resolves input text from CLI argument, file, or stdin (in that order).
///
/// # Errors
///
/// Returns an error if the specified input file cannot be read or stdin reading fails.
pub fn get_input_text(matches: &clap::ArgMatches) -> Result<String> {
    match (
        matches.get_one::<String>("text"),
        matches.get_one::<String>("input-file"),
    ) {
        (Some(text), _) => Ok(text.clone()),
        (None, Some(file_path)) => read_input_file(file_path),
        (None, None) => read_stdin_trimmed(),
    }
}
