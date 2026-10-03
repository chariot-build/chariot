use std::{
    collections::{BTreeSet, HashMap, HashSet},
    num::NonZero,
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use chariot_core::{
    CoreContext,
    config::{
        Config,
        package::{Package, PackagePlatform},
        source::Source,
    },
    executor::{BuildManager, FailureMode},
    graph::BuildGraphBuilder,
    tracer::Tracer,
    workdir::WorkDirectory,
};
use chariot_rootfs::{CachedPkgSet, DEFAULT_MANIFESTS_URL, ManifestFetchSpec, RootFS};
use log::{info, warn};

use crate::{
    args::CommonBuildOptions,
    cache::Cache,
    cli_config::CliConfig,
    config::{ResolvedProfile, resolve_profile},
    terminal::Terminal,
};

pub fn find_package<'a>(config: &'a Config, platform: PackagePlatform, name: &str) -> Result<&'a Arc<Package>> {
    config
        .packages
        .iter()
        .find(|pkg| pkg.platform == platform && pkg.name == name)
        .with_context(|| format!("Could not find {} package `{}`", platform.to_string(), name))
}

pub fn run_build<'a>(
    ctx: &'a CoreContext,
    tracer: Arc<dyn Tracer>,
    root_packages: &[&Arc<Package>],
    root_sources: &[&Arc<Source>],
    failure_mode: FailureMode,
    worker_count: NonZero<usize>,
) -> Result<BuildManager<'a>> {
    let mut builder = BuildGraphBuilder::new();
    for &pkg in root_packages {
        builder.add_root_package(pkg);
    }
    for &source in root_sources {
        builder.add_root_source(source);
    }
    let graph = builder.finish();

    let manager = BuildManager::new(ctx, graph, tracer);
    let report = manager.execute(failure_mode, worker_count);
    if !report.is_success() {
        bail!(
            "build failed: {} task(s) failed, {} skipped (see above for details)",
            report.failed.len(),
            report.skipped.len()
        );
    }

    Ok(manager)
}

pub fn prepare_build(
    cache: &Cache,
    build_opts: CommonBuildOptions,
    local_config: &CliConfig,
    terminal: &Arc<Terminal>,
) -> Result<(CoreContext, Config, WorkDirectory)> {
    let ResolvedProfile {
        workdir_parent,
        local_sources_workdir,
        base_config,
        config,
    } = resolve_profile(cache, build_opts.config_opts, local_config)?;

    let rootfs = Arc::new(match RootFS::get(&build_opts.rootfs).context("Failed to get rootfs")? {
        None => {
            info!("No rootfs found");

            let bar = terminal.add_bar(format!("Initializing rootfs `{}`", base_config.rootfs.version));

            terminal.set_bar_message(bar, "Downloading...");

            let mut writer = terminal.get_bar_writer(bar);

            let rootfs = RootFS::init(
                &build_opts.rootfs,
                &ManifestFetchSpec {
                    url: base_config.rootfs.url.unwrap_or(String::from(DEFAULT_MANIFESTS_URL)),
                    version: base_config.rootfs.version,
                    hash: base_config.rootfs.hash.clone(),
                },
                &mut writer,
            )
            .context("Failed to initialize rootfs")?;

            terminal.remove_bar(bar);

            info!("Successfully initialized the rootfs");
            rootfs
        }
        Some(rootfs) => {
            let hash = &rootfs.get_manifest_spec().hash;
            let version = &rootfs.get_manifest_spec().version;

            let version_match = version == &base_config.rootfs.version;
            let hash_match = hash == &base_config.rootfs.hash;

            if !version_match && !hash_match {
                bail!(
                    "Rootfs manifest version mismatch (current `{}`, wanted `{}). Delete current rootfs at convenience",
                    hash,
                    base_config.rootfs.hash
                );
            }

            if !version_match {
                warn!("Suspicious rootfs, version mismatch but hashes match")
            }

            if !hash_match {
                bail!("Rootfs hash mismatch, expected `{}`, got `{}`", base_config.rootfs.hash, hash);
            }

            rootfs
        }
    });

    let store = Arc::new(cache.open_store()?);
    let ledger = Arc::new(cache.open_ledger()?);
    let build_cache = Arc::new(cache.open_build_cache()?);

    let mut binary_to_pkgset: HashMap<&str, Option<Arc<CachedPkgSet>>> = HashMap::new();
    for binary in ["bsdtar", "git", "patch", "sha256sum", "wget"] {
        let Some(pkg) = rootfs.lookup_package_of_binary(binary) else {
            bail!("This rootfs manifest is missing a required package mapping for the `{}` binary", binary);
        };

        let bar = terminal.add_bar(format!("Fetching {} package set", pkg));
        let mut writer = terminal.get_bar_writer(bar);

        binary_to_pkgset.insert(binary, CachedPkgSet::get(&rootfs, &None, &BTreeSet::from([pkg]), &mut writer)?);

        terminal.remove_bar(bar);
    }

    let mut build_cache_enabled = HashSet::new();
    for (platform, name, pkg) in local_config
        .pkgs
        .iter()
        .map(|(name, pkg)| (PackagePlatform::Target, name, pkg))
        .chain(local_config.tools.iter().map(|(name, tool)| (PackagePlatform::Host, name, tool)))
    {
        if pkg.enable_build_cache {
            build_cache_enabled.insert((platform, name.clone()));
        }
    }

    let ctx = CoreContext {
        build_cache_enabled,
        parallelism: build_opts.parallelism,
        bsdtar_pkgset: binary_to_pkgset.remove("bsdtar").unwrap(),
        git_pkgset: binary_to_pkgset.remove("git").unwrap(),
        patch_pkgset: binary_to_pkgset.remove("patch").unwrap(),
        sha256sum_pkgset: binary_to_pkgset.remove("sha256sum").unwrap(),
        wget_pkgset: binary_to_pkgset.remove("wget").unwrap(),
        store,
        ledger,
        workdir_parent,
        build_cache,
        rootfs,
    };

    Ok((ctx, config, local_sources_workdir))
}
