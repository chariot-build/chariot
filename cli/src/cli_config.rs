use std::{
    collections::HashMap,
    fs::{exists, read_to_string},
    path::{Path, PathBuf},
};

use anyhow::Result;
use chariot_config::SourceOverride;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum OverrideConfig {
    Simple(PathBuf),
    Detailed {
        path: PathBuf,

        #[serde(default)]
        patch: bool,

        #[serde(default)]
        prepare: bool,
    },
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct PackageConfig {
    pub enable_build_cache: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct CliConfig {
    pub pkgs: HashMap<String, PackageConfig>,
    pub tools: HashMap<String, PackageConfig>,
    pub source_overrides: HashMap<String, OverrideConfig>,
    pub disable_pruning: bool,
}

impl CliConfig {
    pub fn get_source_overrides(&self) -> Vec<SourceOverride> {
        self.source_overrides
            .iter()
            .map(|(name, config)| match config {
                OverrideConfig::Simple(path) => SourceOverride {
                    name: name.clone(),
                    path: path.clone(),
                    patched: false,
                    prepared: false,
                },
                OverrideConfig::Detailed { path, patch, prepare } => SourceOverride {
                    name: name.clone(),
                    path: path.clone(),
                    patched: *patch,
                    prepared: *prepare,
                },
            })
            .collect()
    }
}

pub fn parse_cli_config(path: impl AsRef<Path>) -> Result<CliConfig> {
    if !exists(&path)? {
        return Ok(CliConfig::default());
    }

    let data = read_to_string(&path)?;
    let config = toml::from_str::<CliConfig>(&data)?;
    Ok(config)
}
