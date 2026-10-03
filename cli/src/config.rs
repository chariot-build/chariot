use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use chariot_config::{
    DEFAULT_LUA_CONFIG_PATH, SourceOverride,
    base::{BaseConfig, read_base_config},
    lua::eval_lua_config,
};
use chariot_core::{
    DEFAULT_TARGET_PREFIX, collect_all_hashes,
    config::{Config, GlobalEnvironment},
    workdir::{WorkDirectory, WorkDirectoryParent},
};
use chariot_util::fs::join_soft;
use dialoguer::Confirm;

use crate::{args::ConfigOptions, cache::Cache, cli_config::CliConfig};

pub struct ResolvedProfile {
    pub workdir_parent: Arc<WorkDirectoryParent>,
    pub local_sources_workdir: WorkDirectory,
    pub base_config: BaseConfig,
    pub config: Config,
}

pub fn resolve_profile(cache: &Cache, config_opts: ConfigOptions, local_config: &CliConfig) -> Result<ResolvedProfile> {
    let options = HashMap::from_iter(config_opts.option);

    let workdir_parent = cache.open_workdir_parent()?;
    let local_sources_workdir = WorkDirectory::create(&workdir_parent)?;

    let (base_config, config_dir) = read_base_config_and_dir(&config_opts.base_config)?;

    let config = load_profile_config(
        &base_config,
        &config_dir,
        config_opts.arch.clone(),
        options.clone(),
        local_sources_workdir.path(),
        local_config.get_source_overrides(),
    )?;

    let hashes = hash_config(&config);

    if !cache
        .1
        .profile_cache_hashes(false, &config_opts.arch, &options.iter().collect(), &hashes)?
    {
        let ok = config_opts.allow_new_profiles
            || Confirm::new()
                .default(true)
                .with_prompt("Detected a new profile (profile describes a specific permutation of architecture and options). Proceed?")
                .interact()?;

        if !ok {
            bail!("Canceled by user");
        }

        cache
            .1
            .profile_cache_hashes(true, &config_opts.arch, &options.iter().collect(), &hashes)?;
    }

    Ok(ResolvedProfile {
        workdir_parent,
        local_sources_workdir,
        base_config,
        config,
    })
}

pub fn read_base_config_and_dir(base_config_path: &Path) -> Result<(BaseConfig, PathBuf)> {
    let base_config_path = base_config_path
        .canonicalize()
        .context("Failed to canonicalize (find absolute path of) base config")?;

    let config_dir = base_config_path
        .parent()
        .context("Failed to resolve directory of base config")?
        .to_path_buf();

    let base_config = read_base_config(&base_config_path).context("Failed to read base config")?;

    Ok((base_config, config_dir))
}

pub fn load_profile_config(
    base_config: &BaseConfig,
    config_dir: &Path,
    arch: String,
    options: HashMap<String, String>,
    local_sources_workdir: impl AsRef<Path>,
    source_overrides: Vec<SourceOverride>,
) -> Result<Config> {
    let target_prefix = base_config.target_prefix.clone().unwrap_or_else(|| String::from(DEFAULT_TARGET_PREFIX));

    let global_environment = Arc::new(GlobalEnvironment {
        global_environment_variables: base_config.global_environment_variables.clone(),
        global_native_packages: base_config.global_native_packages.clone(),
        rootfs_manifest_hash: base_config.rootfs.hash.clone(),
        target_prefix,
        target_arch: arch,
    });

    let lua_config_path = join_soft(config_dir, base_config.lua_root.clone().unwrap_or(PathBuf::from(DEFAULT_LUA_CONFIG_PATH)));

    eval_lua_config(
        &lua_config_path,
        config_dir,
        global_environment,
        options,
        local_sources_workdir,
        source_overrides,
    )
    .context("Failed to evaluate lua config")
}

pub fn hash_config(config: &Config) -> HashSet<(String, u128)> {
    collect_all_hashes(config)
        .into_iter()
        .map(|(cat, hash)| (cat.to_string(), hash))
        .collect()
}
