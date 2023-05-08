use std::collections::HashSet;

use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct FuzzConfig {
    pub binary: BinaryConfig,

    pub input: InputOptions,

    #[serde(default)]
    pub output: OutputOptions,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BinaryConfig {
    pub path: String,
    pub pass_style: PassStyle,

    #[serde(default)]
    pub interesting_codes: ExitCodeFilter,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum InputOptions {
    Grammar { grammar: String },
    Seeds { seeds: String },
}

#[derive(Copy, Clone, Debug, Deserialize, Default)]
pub struct StdinFuzzingOptions {}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PassStyle {
    #[default]
    Stdin,
    File,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SeedOptions {
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(untagged)]
pub enum ExitCodeFilter {
    #[default]
    Any,
    Set(HashSet<i32>),
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

#[derive(Clone, Debug, Deserialize)]
pub struct OutputOptions {
    pub directory: String,
}

impl Default for OutputOptions {
    fn default() -> Self {
        Self {
            directory: "output".to_string(),
        }
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
