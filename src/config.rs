use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub text_splitter: TextSplitterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSplitterConfig {
    #[serde(default = "default_delimiters")]
    pub delimiters: Vec<String>,
    #[serde(default = "default_max_length")]
    pub max_length: usize,
}

impl Default for TextSplitterConfig {
    fn default() -> Self {
        Self {
            delimiters: default_delimiters(),
            max_length: default_max_length(),
        }
    }
}

fn default_delimiters() -> Vec<String> {
    vec![
        "。".to_string(),
        "！".to_string(),
        "？".to_string(),
        "．".to_string(),
        "\n".to_string(),
    ]
}

fn default_max_length() -> usize {
    100
}
