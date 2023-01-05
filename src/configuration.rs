use std::collections::HashSet;

use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct FuzzConfig {
    pub binary: BinaryConfig,
    pub stdin: Option<StdinFuzzingOptions>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BinaryConfig {
    pub path: String,

    #[serde(default)]
    pub interesting_codes: ExitCodeFilter,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StdinFuzzingOptions {
    #[serde(default = "default_stdin_limit")]
    pub limit: usize,
}

fn default_stdin_limit() -> usize {
    10_000
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ExitCodeFilter {
    Any,
    Set(HashSet<i32>),
}

impl Default for ExitCodeFilter {
    fn default() -> Self {
        ExitCodeFilter::Any
    }
}

impl ExitCodeFilter {
    pub fn match_code(&self, code: i32) -> bool {
        match self {
            ExitCodeFilter::Any => true,
            ExitCodeFilter::Set(s) => s.contains(&code),
        }
    }

    pub fn accepts_any(&self) -> bool {
        matches!(self, ExitCodeFilter::Any)
    }
}

pub enum ConfigReadError {
    ReadError(std::io::Error),
    ParseError(toml::de::Error),
}

pub fn load_config<P: AsRef<std::path::Path>>(path: P) -> Result<FuzzConfig, ConfigReadError> {
    let config = std::fs::read_to_string(path).map_err(ConfigReadError::ReadError)?;

    toml::from_str::<FuzzConfig>(&config).map_err(ConfigReadError::ParseError)
}
