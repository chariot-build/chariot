use std::{collections::HashSet, num::NonZero, sync::Arc};

use chariot_rootfs::{CachedPkgSet, RootFS};

use crate::{
    buildcache::BuildCache,
    config::{Config, package::PackagePlatform},
    jobserver::JobServer,
    ledger::Ledger,
    store::Store,
    workdir::WorkDirectoryParent,
};

pub mod buildcache;
pub mod config;
pub mod execenv;
pub mod executor;
pub mod graph;
pub mod jobserver;
pub mod ledger;
pub mod package;
pub mod source;
pub mod store;
pub mod tracer;
pub mod workdir;
pub mod xbps;

pub const HOST_ARCH: &str = std::env::consts::ARCH;
pub const HOST_PREFIX: &str = "/usr/local";
pub const DEFAULT_TARGET_PREFIX: &str = "/usr";

pub struct CoreContext {
    pub build_cache_enabled: HashSet<(PackagePlatform, String)>,
    pub parallelism: NonZero<usize>,
    pub jobserver: JobServer,
    pub rootfs: Arc<RootFS>,
    pub store: Arc<Store>,
    pub ledger: Arc<Ledger>,
    pub build_cache: Arc<BuildCache>,
    pub workdir_parent: Arc<WorkDirectoryParent>,
    pub git_pkgset: Option<Arc<CachedPkgSet>>,
    pub wget_pkgset: Option<Arc<CachedPkgSet>>,
    pub sha256sum_pkgset: Option<Arc<CachedPkgSet>>,
    pub bsdtar_pkgset: Option<Arc<CachedPkgSet>>,
    pub unzip_pkgset: Option<Arc<CachedPkgSet>>,
    pub patch_pkgset: Option<Arc<CachedPkgSet>>,
}

pub fn collect_all_hashes(config: &Config) -> HashSet<(&'static str, u128)> {
    let mut hashes = HashSet::new();
    for pkg in &config.packages {
        hashes.insert(("install", pkg.get_content_hash()));
        hashes.insert(("pkg", pkg.get_package_hash()));
    }
    for src in &config.sources {
        let base_hash = src.get_base_hash();
        let patch_hash = src.get_patch_hash(base_hash);
        let prepare_hash = src.get_prepare_hash(src.get_prepare_base_hash(patch_hash));
        hashes.insert(("source.base", base_hash));
        hashes.insert(("source.patch", patch_hash));
        hashes.insert(("source.prepare", prepare_hash));
    }
    hashes
}

pub fn resolve_effective_hashes<'a>(
    ledger: &Ledger,
    unresolved_hashes: impl Iterator<Item = (&'a str, u128)>,
) -> Result<HashSet<(&'a str, u128)>, rusqlite::Error> {
    let mut resolved_hashes = HashSet::new();
    for (category, hash) in unresolved_hashes {
        match category {
            "install" | "pkg" | "source.prepare" => {
                if let Some(effective_hash) = ledger.lookup(category, hash)? {
                    resolved_hashes.insert((category, effective_hash));
                }
            }
            _ => {
                resolved_hashes.insert((category, hash));
            }
        }
    }
    Ok(resolved_hashes)
}
