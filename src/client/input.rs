use anyhow::Result;
use std::fs;
use std::io::{self, Read};

pub fn get_input_text(matches: &clap::ArgMatches) -> Result<String> {
    // Command line argument
    if let Some(text) = matches.get_one::<String>("text") {
        return Ok(text.clone());
    }

    // File input
    if let Some(file_path) = matches.get_one::<String>("input-file") {
        if file_path == "-" {
            // Read from stdin
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            return Ok(buffer.trim().to_string());
        } else {
            // Read from file
            return Ok(fs::read_to_string(file_path)?);
        }
    }

    // Default to stdin if no text specified
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}
