use std::{io::stdout, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use chariot_core::{config::package::PackagePlatform, xbps::package_install};
use chariot_util::fs::make_path;

use crate::{
    args::InstallOptions,
    build::{find_package, prepare_build, run_build},
    cache::{Cache, prune_store_and_ledger},
    cli_config::CliConfig,
    terminal::Terminal,
    tracer::CliTracer,
};

pub fn run(install_opts: InstallOptions, local_config: &CliConfig) -> Result<()> {
    let terminal = Arc::new(Terminal::new());
    let render_handle = terminal.spawn_renderer(Duration::from_millis(100));

    let cache = Cache::get(&install_opts.common_build_opts.config_opts.cache)?;

    let mode = install_opts.execution_opts.failure_mode();
    let (ctx, config, _local_sources_workdir) = prepare_build(&cache, install_opts.common_build_opts, local_config, Some(&terminal))?;

    let platform = if install_opts.tool {
        PackagePlatform::Host
    } else {
        PackagePlatform::Target
    };
    let mut selected_packages = Vec::new();
    for name in &install_opts.packages {
        selected_packages.push(find_package(&config, platform, name)?);
    }

    let manager = match run_build(&ctx, Arc::new(CliTracer::new(terminal.clone())), &selected_packages, &[], mode) {
        Ok(manager) => manager,
        Err(err) => return Err(err),
    };

    drop(render_handle);

    make_path(&install_opts.dest)?;

    for selected_package in selected_packages {
        let paths = manager.package_install_paths(selected_package);

        package_install(
            &ctx,
            None,
            &selected_package.name,
            &selected_package.version,
            selected_package.revision,
            selected_package.get_arch(),
            paths,
            &PathBuf::from(&install_opts.dest),
            false,
            install_opts.force,
            &mut stdout(),
        )?;
    }

    if !local_config.disable_pruning {
        prune_store_and_ledger(&ctx.store, &ctx.ledger, &cache.1.all_cached_hashes()?)?;
        ctx.build_cache.prune(&cache.1.all_cached_packages()?)?;
    }

    Ok(())
}
