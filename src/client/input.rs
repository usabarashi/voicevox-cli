use anyhow::Result;
use std::fs;
use std::io::{self, Read};

fn read_stdin_trimmed() -> Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}

/// Resolves input text from CLI argument, file, or stdin (in that order).
///
/// # Errors
///
/// Returns an error if the specified input file cannot be read or stdin reading fails.
pub fn get_input_text(matches: &clap::ArgMatches) -> Result<String> {
    matches
        .get_one::<String>("text")
        .cloned()
        .map(Ok)
        .or_else(|| {
            matches.get_one::<String>("input-file").map(|file_path| {
                if file_path == "-" {
                    read_stdin_trimmed()
                } else {
                    fs::read_to_string(file_path).map_err(Into::into)
                }
            })
        })
        .unwrap_or_else(read_stdin_trimmed)
}
