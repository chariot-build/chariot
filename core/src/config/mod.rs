use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::config::{package::Package, source::Source};

pub mod package;
pub mod script;
pub mod source;

/// Describes the config version. If the config format changes
/// in a backwards incompatible way, this version should be bumped.
const CONFIG_VERSION: u64 = 9;

#[derive(Debug, Default, Clone)]
pub struct Dependencies {
    pub native: BTreeSet<String>,
    pub sources: Vec<Arc<Source>>,
    pub tools: Vec<Arc<Package>>,
    pub packages: Vec<Arc<Package>>,
}

#[derive(Debug)]
pub struct GlobalEnvironment {
    pub rootfs_manifest_hash: String,
    pub global_environment_variables: BTreeMap<String, String>,
    pub global_native_packages: BTreeSet<String>,
    pub target_arch: String,
    pub target_prefix: String,
}

pub struct Config {
    pub global_env: Arc<GlobalEnvironment>,

    /// all packages referenced by config must be here
    pub packages: Vec<Arc<Package>>,

    /// all sources referenced by config must be here
    pub sources: Vec<Arc<Source>>,
}
