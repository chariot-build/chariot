use std::collections::HashSet;

use anyhow::{Context, Result};
use chariot_core::workdir::WorkDirectory;
use log::info;

use crate::{
    args::{CacheCommand, CacheOptions},
    cache::{Cache, prune_store_and_ledger},
    cli_config::CliConfig,
    config::{hash_config, load_profile_config, read_base_config_and_dir},
};

pub fn run(options: CacheOptions, local_config: &CliConfig) -> Result<()> {
    let CacheOptions { cache, command } = options;

    let cache = Cache::get(&cache)?;

    match command {
        CacheCommand::Purge => {
            let store = cache.open_store()?;
            let ledger = cache.open_ledger()?;

            store.prune_store(HashSet::new()).context("Failed to purge store")?;
            ledger.prune(HashSet::new()).context("Failed to purge ledger")?;
        }
        CacheCommand::Gc {
            base_config: base_config_path,
        } => {
            let store = cache.open_store()?;
            let ledger = cache.open_ledger()?;
            let workdir_parent = cache.open_workdir_parent()?;
            let local_sources_workdir = WorkDirectory::create(&workdir_parent)?;

            let (base_config, config_dir) = read_base_config_and_dir(&base_config_path)?;

            let known_profiles = cache.1.all_cached_profiles()?;
            for (arch, options) in known_profiles {
                let config = load_profile_config(
                    &base_config,
                    &config_dir,
                    arch.clone(),
                    options.clone(),
                    local_sources_workdir.path(),
                    local_config.get_source_overrides(),
                )?;

                cache
                    .1
                    .profile_cache_hashes(false, &arch, &options.iter().collect(), &hash_config(&config))?;
            }

            prune_store_and_ledger(&store, &ledger, &cache.1.all_cached_hashes()?)?;
        }
        CacheCommand::ListLedger => {
            let ledger = cache.open_ledger()?;
            let records = ledger.list().context("Failed to list ledger records")?;
            let max_category_length = records.iter().map(|(cat, ..)| cat.len()).max().unwrap_or(0).max(8);
            info!(
                "{:<cat_width$} {:<32} {:<32}",
                "category",
                "input_hash",
                "effective_hash",
                cat_width = max_category_length
            );
            info!("{}", "-".repeat(max_category_length + 66));
            for (category, hash, effective_hash) in records {
                info!(
                    "{:<cat_width$} {:<32x} {:<32x}",
                    category,
                    hash,
                    effective_hash,
                    cat_width = max_category_length
                );
            }
        }
    }

    Ok(())
}
