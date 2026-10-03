use std::{sync::Arc, time::Duration};

use anyhow::Result;
use chariot_core::{config::package::PackagePlatform, tracer::Tracer};

use crate::{
    args::BuildOptions,
    build::{find_package, prepare_build, run_build},
    cache::{Cache, prune_store_and_ledger},
    cli_config::CliConfig,
    terminal::Terminal,
    tracer::CliTracer,
};

pub fn run(build_opts: BuildOptions, local_config: &CliConfig) -> Result<()> {
    let terminal = Arc::new(Terminal::new());
    let render_handle = terminal.spawn_renderer(Duration::from_millis(100));

    let cache = Cache::get(&build_opts.common_build_opts.config_opts.cache)?;

    let worker_count = build_opts.execution_opts.worker_count;
    let mode = build_opts.execution_opts.failure_mode();
    let (ctx, config, cached_hashes, _local_sources_workdir) = prepare_build(&cache, build_opts.common_build_opts, local_config, &terminal)?;

    let platform = if build_opts.tool {
        PackagePlatform::Host
    } else {
        PackagePlatform::Target
    };
    let mut selected_packages = Vec::new();
    for name in &build_opts.packages {
        selected_packages.push(find_package(&config, platform, name)?);
    }

    let tracer: Arc<dyn Tracer> = Arc::new(CliTracer::new(terminal.clone()));

    if let Err(err) = run_build(&ctx, tracer, &selected_packages, &[], mode, worker_count) {
        drop(render_handle);
        return Err(err);
    }

    if !local_config.disable_pruning {
        prune_store_and_ledger(&ctx.store, &ctx.ledger, &cached_hashes)?;
        ctx.build_cache.prune(ctx.build_cache_enabled)?;
    }

    Ok(())
}
