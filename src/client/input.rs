use anyhow::Result;
use std::fs;
use std::io::{self, Read};

pub fn get_input_text(matches: &clap::ArgMatches) -> Result<String> {
    if let Some(text) = matches.get_one::<String>("text") {
        return Ok(text.clone());
    }

    if let Some(file_path) = matches.get_one::<String>("input-file") {
        if file_path == "-" {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            return Ok(buffer.trim().to_string());
        } else {
            return Ok(fs::read_to_string(file_path)?);
        }
    }

    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer.trim().to_string())
}
