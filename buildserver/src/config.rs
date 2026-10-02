use std::collections::HashMap;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct GitRepositoryConfig {
    pub url: String,
    pub branch: Option<String>,
}

#[derive(Deserialize)]
pub struct BuildProfile {
    pub target_arch: String,

    #[serde(default)]
    pub options: HashMap<String, String>,
}

#[derive(Deserialize)]
pub struct ProjectConfig {
    pub git_repository: GitRepositoryConfig,
    pub base_config_path: Option<String>,

    #[serde(default)]
    pub disabled: bool,

    #[serde(default)]
    pub profiles: Vec<BuildProfile>,

    #[serde(default)]
    pub build_packages: Vec<String>,

    #[serde(default)]
    pub build_tools: Vec<String>,
}

#[derive(Deserialize)]
pub struct BuildServerConfig {
    pub projects: HashMap<String, ProjectConfig>,
    pub interval: u64,
}
