use fs_err::File;
use std::io::{BufRead, BufReader};

use anyhow::Result;

use crate::errors::ShellError;

// Represents any settings for the shell, most of which can be configured by the user
pub struct Configuration {
    // The truncation length for the prompt
    pub truncation_factor: Option<usize>,
    // How many directories to store in the back/forward history
    pub history_limit: Option<usize>,
    // Whether or not to print out full error messages and status codes when a command fails
    pub show_errors: bool,
    // Whether the prompt should be displayed in a single line or multiple lines
    pub multi_line_prompt: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            truncation_factor: None,
            history_limit: None,
            show_errors: true,
            multi_line_prompt: false,
        }
    }
}

impl Configuration {
    // Scans a configuration file for settings and updates the configuration accordingly
    pub fn from_file(filename: &str) -> Result<Self> {
        let filename = filename.to_string();

        let mut config = Self::default();
        let file = File::open(filename.clone())
            .map_err(|_| ShellError::FailedToOpenConfigFile(filename.clone()))?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line.map_err(|_| ShellError::FailedToOpenConfigFile(filename.clone()))?;
            let tokens = line.split(": ").collect::<Vec<&str>>();
            if tokens.len() != 2 {
                return Err(ShellError::FailedToReadConfigFile(filename).into());
            }

            let (key, value) = (tokens[0], tokens[1]);

            // ? Should these be underscores instead of hyphens?
            match key {
                "truncation-factor" => {
                    if let Ok(length) = value.parse::<usize>() {
                        config.truncation_factor = Some(length);
                    } else if value == "false" {
                        config.truncation_factor = None;
                    }
                }
                "history-limit" => {
                    if let Ok(limit) = value.parse::<usize>() {
                        config.history_limit = Some(limit);
                    } else if value == "false" {
                        config.history_limit = None;
                    }
                }
                "show-errors" => {
                    if let Ok(show) = value.parse::<bool>() {
                        config.show_errors = show;
                    }
                }
                "multi-line-prompt" => {
                    if let Ok(multi_line) = value.parse::<bool>() {
                        config.multi_line_prompt = multi_line;
                    }
                }
                _ => return Err(ShellError::FailedToReadConfigFile(filename).into()),
            }
        }

        Ok(config)
    }
}
