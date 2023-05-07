use std::collections::HashSet;

use serde_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct FuzzConfig {
    pub binary: BinaryConfig,

    pub grammar: GrammarOptions,

    #[serde(default)]
    pub stdin: StdinFuzzingOptions,

    #[serde(default)]
    pub generation: GenerationOptions,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BinaryConfig {
    pub path: String,

    #[serde(default)]
    pub interesting_codes: ExitCodeFilter,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GrammarOptions {
    pub path: String,
}

#[derive(Copy, Clone, Debug, Deserialize, Default)]
pub struct StdinFuzzingOptions {
    pub pass_style: PassStyle,
}

#[derive(Copy, Clone, Debug, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PassStyle {
    #[default]
    Stdin,
    File,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GenerationOptions {
    #[serde(default = "default_population_size")]
    pub population: usize,
    #[serde(default = "default_sample_limit")]
    pub sample_limit: usize,
}

impl Default for GenerationOptions {
    fn default() -> Self {
        Self {
            population: default_population_size(),
            sample_limit: default_sample_limit(),
        }
    }
}

fn default_population_size() -> usize {
    1_000
}

fn default_sample_limit() -> usize {
    100
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

pub enum ConfigReadError {
    ReadError(std::io::Error),
    ParseError(toml::de::Error),
}

pub fn load_config<P: AsRef<std::path::Path>>(path: P) -> Result<FuzzConfig, ConfigReadError> {
    let config = std::fs::read_to_string(path).map_err(ConfigReadError::ReadError)?;

    toml::from_str::<FuzzConfig>(&config).map_err(ConfigReadError::ParseError)
}
