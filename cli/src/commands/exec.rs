use std::{
    collections::BTreeSet,
    io::{stderr, stdout},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use chariot_core::{
    buildcache::BuildDirectory,
    config::{
        package::{Package, PackagePlatform},
        script::Script,
        source::Source,
    },
    execenv::ExecEnv,
    tracer::{CapturingLogger, Tracer},
    workdir::WorkDirectory,
};
use chariot_rootfs::{CachedPkgSet, StderrTarget};
use chariot_runtime::{Mount, MountKind, OverlayUpperDirectory};
use chariot_util::fs::make_path;

use crate::{
    args::ExecOptions,
    build::{find_package, prepare_build, run_build},
    cache::Cache,
    cli_config::CliConfig,
    terminal::Terminal,
    tracer::CliTracer,
};

pub fn run(exec_options: ExecOptions, local_config: &CliConfig) -> Result<()> {
    let terminal = Arc::new(Terminal::new());
    let render_handle = terminal.spawn_renderer(Duration::from_millis(100));

    let cache = Cache::get(&exec_options.common_build_opts.config_opts.cache)?;

    let worker_count = exec_options.execution_opts.worker_count;
    let mode = exec_options.execution_opts.failure_mode();
    let (ctx, config, _local_sources_workdir) = prepare_build(&cache, exec_options.common_build_opts, local_config, &terminal)?;

    let mut packages = exec_options
        .pkg
        .iter()
        .map(|name| find_package(&config, PackagePlatform::Target, name))
        .collect::<Result<Vec<_>>>()?;

    let mut tools = exec_options
        .tool
        .iter()
        .map(|name| find_package(&config, PackagePlatform::Host, name))
        .collect::<Result<Vec<_>>>()?;

    let build_env_pkg = exec_options
        .build_env
        .as_deref()
        .map(|name| find_package(&config, PackagePlatform::Target, name).with_context(|| format!("Failed to resolve build_env package `{}`", name)))
        .transpose()?;

    let native_pkgs: BTreeSet<&str> = exec_options
        .native_pkg
        .iter()
        .map(String::as_str)
        .chain(build_env_pkg.iter().flat_map(|pkg| pkg.dependencies.native.iter().map(String::as_str)))
        .collect();
    let pkgset = CachedPkgSet::get(&ctx.rootfs, &None, &native_pkgs, &mut stderr())?;

    if let Some(pkg) = build_env_pkg {
        for dep in &pkg.dependencies.packages {
            packages.push(dep);
        }
        for dep in &pkg.dependencies.tools {
            tools.push(dep);
        }
    }

    let source_deps: Vec<&Arc<Source>> = build_env_pkg.map(|pkg| pkg.dependencies.sources.iter().collect()).unwrap_or_default();

    let tracer: Arc<dyn Tracer> = Arc::new(CliTracer::new(terminal.clone()));

    let root_packages: Vec<&Arc<Package>> = packages.iter().chain(tools.iter()).copied().collect();
    let manager = match run_build(&ctx, tracer, &root_packages, &source_deps, mode, worker_count) {
        Ok(manager) => manager,
        Err(err) => {
            drop(render_handle);
            return Err(err);
        }
    };

    let target_packages: Vec<(&Package, Vec<PathBuf>)> = packages.iter().map(|&pkg| (pkg.as_ref(), manager.package_install_paths(pkg))).collect();
    let host_tools: Vec<(&Package, Vec<PathBuf>)> = tools.iter().map(|&pkg| (pkg.as_ref(), manager.package_install_paths(pkg))).collect();
    let sources: Vec<(&Source, Vec<PathBuf>)> = source_deps.iter().map(|source| (source.as_ref(), manager.source_paths(source))).collect();

    let mountpoint_overlay_workdir = WorkDirectory::create(&ctx.workdir_parent)?;
    let mountpoint_overlay_overlay_path = mountpoint_overlay_workdir.path().join("mountpoint_overlay");
    let mountpoint_overlay_work_path = mountpoint_overlay_workdir.path().join("work");
    make_path(&mountpoint_overlay_overlay_path)?;
    make_path(&mountpoint_overlay_work_path)?;
    let mountpoint_overlay = OverlayUpperDirectory {
        upper_directory: mountpoint_overlay_overlay_path,
        work_directory: mountpoint_overlay_work_path,
    };

    let exec_env = ExecEnv::assemble(
        &ctx,
        || Box::new(CapturingLogger::new(stderr())),
        pkgset,
        &sources,
        &target_packages,
        &host_tools,
        false,
        Some(mountpoint_overlay),
    )?;

    let mut mounts = exec_options
        .mount
        .into_iter()
        .map(|(from, to, read_only, is_file)| Mount {
            dest: PathBuf::from(to),
            kind: MountKind::Bind {
                from: PathBuf::from(from),
                read_only,
                is_file,
            },
        })
        .collect::<Vec<_>>();

    let mut _build_directories = Vec::new();
    for (dest, pkg_name) in exec_options.build_dir {
        let build_dir = BuildDirectory::get_ro(&ctx.build_cache, PackagePlatform::Target, &config.global_env.target_arch, &pkg_name)
            .with_context(|| format!("Failed to get build directory for package `{}`", pkg_name))?;

        mounts.push(Mount {
            dest: PathBuf::from(dest),
            kind: MountKind::Bind {
                from: build_dir.path(),
                read_only: true,
                is_file: false,
            },
        });

        _build_directories.push(build_dir);
    }

    let script = Script::new(exec_options.language, exec_options.command);

    drop(render_handle);

    let mut stderr_handle = stderr();
    let mut stdout_handle = stdout();
    exec_env.exec(
        exec_options.cwd,
        mounts.iter().collect(),
        &exec_options.env_var.iter().map(|(var, value)| (var.as_str(), value.as_str())).collect(),
        exec_options.stdin,
        match exec_options.no_stdout {
            true => None,
            false => Some(&mut stdout_handle),
        },
        match exec_options.no_stderr {
            true => StderrTarget::Discard,
            false => StderrTarget::Capture(&mut stderr_handle),
        },
        script.command(),
    )?;

    Ok(())
}
