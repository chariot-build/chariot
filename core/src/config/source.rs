use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};

use xxhash_rust::xxh3::Xxh3;

use crate::config::{CONFIG_VERSION, Dependencies, GlobalEnvironment, script::Script};

#[derive(Debug, Hash)]
pub struct Archive {
    pub url: String,
    pub checksum: String,
    pub kind: ArchiveKind,
    pub compression: ArchiveCompression,
}

#[derive(Debug, Hash)]
pub enum ArchiveKind {
    Tar,
}

#[derive(Debug, Hash)]
pub enum ArchiveCompression {
    Xz,
    Gzip,
    Bzip2,
}

#[derive(Debug, Hash)]
pub struct GitSource {
    pub url: String,
    pub revision: String,
}

/// Creator must ensure the directory does not change until config is
/// dropped. The hash will not be validated, it is taken at face value.
#[derive(Debug)]
pub struct LocalSource {
    pub original_path: PathBuf,
    pub cached_path: PathBuf,
    pub hash: u128,
}

impl Hash for LocalSource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u128(self.hash);
    }
}

#[derive(Debug, Hash)]
pub enum SourceBase {
    Archive(Archive),
    Git(GitSource),
    Local(LocalSource),
}

#[derive(Debug, Clone)]
pub struct SourcePrepare {
    pub global_env: Arc<GlobalEnvironment>,
    pub dependencies: Dependencies,
    pub environment_variables: BTreeMap<String, String>,
    pub script: Script,
}

#[derive(Debug)]
pub struct Source {
    pub name: String,
    pub base: SourceBase,
    pub patches: Vec<String>,
    pub prepare: Option<SourcePrepare>,
}

impl Source {
    pub fn get_base_hash(&self) -> u128 {
        let mut hasher = Xxh3::new();
        CONFIG_VERSION.hash(&mut hasher);
        self.base.hash(&mut hasher);
        hasher.digest128()
    }

    pub fn get_patch_hash(&self, base_hash: u128) -> u128 {
        let mut hasher = Xxh3::new();
        base_hash.hash(&mut hasher);
        self.patches.hash(&mut hasher);
        hasher.digest128()
    }

    pub fn get_prepare_base_hash(&self, patch_hash: u128) -> u128 {
        let mut hasher = Xxh3::new();
        patch_hash.hash(&mut hasher);
        if let Some(prepare) = &self.prepare {
            prepare.global_env.rootfs_manifest_hash.hash(&mut hasher);
            prepare.global_env.global_environment_variables.hash(&mut hasher);
            prepare.global_env.global_native_packages.hash(&mut hasher);
            prepare.environment_variables.hash(&mut hasher);
            prepare.script.hash(&mut hasher);
            prepare.dependencies.native.hash(&mut hasher);
        }
        hasher.digest128()
    }

    pub fn get_prepare_hash(&self, prepare_base_hash: u128) -> u128 {
        let mut hasher = Xxh3::new();
        prepare_base_hash.hash(&mut hasher);
        if let Some(prepare) = &self.prepare {
            prepare.dependencies.sources.hash(&mut hasher);
            prepare.dependencies.tools.hash(&mut hasher);
            prepare.dependencies.packages.hash(&mut hasher);
        }
        hasher.digest128()
    }
}

impl Hash for Source {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_usize(self.name.len());
        state.write(self.name.as_bytes());
        state.write_u128(self.get_prepare_hash(self.get_prepare_base_hash(self.get_patch_hash(self.get_base_hash()))));
    }
}
