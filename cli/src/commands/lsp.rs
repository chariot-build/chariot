use std::{
    collections::{BTreeSet, HashMap},
    env::current_dir,
    io::{stderr, stdout},
    iter,
    path::PathBuf,
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use chariot_core::{
    buildcache::BuildDirectory,
    config::{
        package::{Package, PackagePlatform},
        script::{Script, ScriptLanguage},
        source::{Source, SourceBase},
    },
    execenv::{EXECENV_SOURCE_DIRECTORY_PATH, EXECENV_SOURCES_DIRECTORY_PATH, EXECENV_SYSROOT_DIRECTORY_PATH, ExecEnv},
    package::PACKAGE_BUILD_DIR,
    tracer::CapturingLogger,
    workdir::WorkDirectory,
};
use chariot_rootfs::{CachedPkgSet, StderrTarget};
use chariot_runtime::{Mount, MountKind, Overlay};
use log::info;

use crate::{
    args::{LspOptions, SupportedLsp},
    build::{find_package, prepare_build, run_build},
    cache::Cache,
    cli_config::CliConfig,
    tracer::SimpleTracer,
};

pub fn run(lsp_options: LspOptions, local_config: &CliConfig) -> Result<()> {
    let cache = Cache::get(&lsp_options.common_build_opts.config_opts.cache)?;

    let mode = lsp_options.execution_opts.failure_mode();
    let (ctx, config, _local_sources_workdir) = prepare_build(&cache, lsp_options.common_build_opts, local_config, None)?;

    let build_env_pkg = find_package(&config, PackagePlatform::Target, &lsp_options.package)
        .with_context(|| format!("Failed to resolve package `{}`", lsp_options.package))?;

    info!("Getting build environment package set");
    let pkgset = CachedPkgSet::get(&ctx.rootfs, &None, &build_env_pkg.dependencies.native, &mut stderr())?;

    info!("Getting LSP package set");
    let pkgset = CachedPkgSet::get(
        &ctx.rootfs,
        &pkgset,
        &BTreeSet::from(match lsp_options.lsp {
            SupportedLsp::Clangd => {
                let clangd_package = match ctx.rootfs.lookup_package_of_binary("clangd") {
                    None => bail!("Manifest is missing a clangd binary to package mapping"),
                    Some(package) => package,
                };
                [clangd_package]
            }
        }),
        &mut stderr(),
    )?;

    let manager = run_build(
        &ctx,
        Arc::new(SimpleTracer::new()),
        &iter::chain(&build_env_pkg.dependencies.packages, &build_env_pkg.dependencies.tools).collect::<Vec<_>>(),
        &iter::chain(&build_env_pkg.dependencies.sources, &build_env_pkg.source).collect::<Vec<_>>(),
        mode,
    )?;

    let target_packages: Vec<(&Package, Vec<PathBuf>)> = build_env_pkg
        .dependencies
        .packages
        .iter()
        .map(|pkg| (pkg.as_ref(), manager.package_install_paths(&pkg)))
        .collect();
    let host_tools: Vec<(&Package, Vec<PathBuf>)> = build_env_pkg
        .dependencies
        .tools
        .iter()
        .map(|pkg| (pkg.as_ref(), manager.package_install_paths(&pkg)))
        .collect();
    let sources: Vec<(&Source, Vec<PathBuf>)> = build_env_pkg
        .dependencies
        .sources
        .iter()
        .map(|source| (source.as_ref(), manager.source_paths(source)))
        .collect();
    let source = build_env_pkg
        .source
        .as_ref()
        .map(|source| (source.as_ref(), manager.source_paths(source)));

    info!("Assembling execution environment");
    let exec_env = ExecEnv::assemble(
        &ctx,
        || Box::new(CapturingLogger::new(stderr())),
        pkgset,
        source.clone(),
        &sources,
        &target_packages,
        &host_tools,
        true,
        None,
    )?;

    let build_dir = BuildDirectory::get_ro(&ctx.build_cache, build_env_pkg.platform, build_env_pkg.get_arch(), &build_env_pkg.name)
        .context("Failed to get build env")?;

    let build_dir_mount = Mount {
        dest: PathBuf::from(PACKAGE_BUILD_DIR),
        kind: MountKind::Bind {
            from: build_dir.path(),
            read_only: true,
            is_file: false,
        },
    };

    let home_workdir = WorkDirectory::create(&ctx.workdir_parent)?;

    let home_mount = Mount {
        dest: PathBuf::from("/chariot/home"),
        kind: MountKind::Bind {
            from: home_workdir.path(),
            read_only: false,
            is_file: false,
        },
    };

    let base_env = build_env_pkg
        .global_env
        .global_environment_variables
        .iter()
        .chain(&build_env_pkg.environment_variables)
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .chain([
            ("BUILD_DIR", PACKAGE_BUILD_DIR),
            ("PREFIX", build_env_pkg.get_prefix()),
            ("ARCH", build_env_pkg.get_arch()),
            ("HOME", "/chariot/home"),
            ("XDG_CACHE_HOME", "/chariot/home/cache"),
        ])
        .collect();

    let mut merged_source_workdirs = Vec::new();
    let mut mappings = vec![(exec_env.sysroot.path(), PathBuf::from(EXECENV_SYSROOT_DIRECTORY_PATH))];
    let mut source_map_mounts = Vec::new();

    for (source, dest, paths) in iter::chain(
        sources
            .into_iter()
            .map(|(source, paths)| (source, PathBuf::from(EXECENV_SOURCES_DIRECTORY_PATH).join(&source.name), paths)),
        source.map(|(source, paths)| (source, PathBuf::from(EXECENV_SOURCE_DIRECTORY_PATH), paths)),
    ) {
        match lsp_options.source_mappings.iter().find(|(_, name)| name == &source.name) {
            Some((path, _)) => {
                let path = current_dir()?.join(path).canonicalize()?;
                source_map_mounts.push(Mount {
                    dest: dest.clone(),
                    kind: MountKind::Bind {
                        from: path.clone(),
                        read_only: true,
                        is_file: false,
                    },
                });
                mappings.push((path, dest));
            }
            None => {
                if paths.len() > 1 {
                    let source_merge_workdir = WorkDirectory::create(&ctx.workdir_parent)?;

                    info!("Merging source `{}`", source.name);
                    ctx.rootfs.exec(
                        "/",
                        &vec![
                            &Mount {
                                dest: PathBuf::from("/chariot"),
                                kind: MountKind::FS {
                                    fstype: String::from("tmpfs"),
                                },
                            },
                            &Mount {
                                dest: PathBuf::from("/chariot/source"),
                                kind: MountKind::OverlayFS(Overlay {
                                    upper_directory: None,
                                    lower_directories: paths,
                                }),
                            },
                            &Mount {
                                dest: PathBuf::from("/chariot/merged"),
                                kind: MountKind::Bind {
                                    from: source_merge_workdir.path(),
                                    read_only: false,
                                    is_file: false,
                                },
                            },
                        ],
                        &HashMap::<&str, &str>::new(),
                        false,
                        Some(&mut stderr()),
                        StderrTarget::Merge,
                        Script::bash("cp -r /chariot/source /chariot/merged").command(),
                        None,
                        true,
                        None,
                        Vec::new(),
                    )?;

                    mappings.push((source_merge_workdir.path(), dest));

                    merged_source_workdirs.push(source_merge_workdir);
                } else {
                    if let SourceBase::Local(local) = &source.base
                        && paths.len() == 1
                    {
                        source_map_mounts.push(Mount {
                            dest: dest.clone(),
                            kind: MountKind::Bind {
                                from: local.original_path.clone(),
                                read_only: true,
                                is_file: false,
                            },
                        });

                        mappings.push((local.original_path.clone(), dest));
                    } else {
                        mappings.push((paths[0].clone(), dest));
                    }
                }
            }
        }
    }

    let script = match lsp_options.lsp {
        SupportedLsp::Clangd => Script::new(
            ScriptLanguage::Bash,
            format!(
                "clangd --background-index --clang-tidy --path-mappings \"{}\"",
                mappings
                    .iter()
                    .map(|(from, to)| format!("{}={}", from.to_string_lossy(), to.to_string_lossy()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ),
    };

    info!("Running the LSP");
    let mut stderr_handle = stderr();
    let mut stdout_handle = stdout();
    exec_env.exec(
        PACKAGE_BUILD_DIR,
        source_map_mounts.iter().chain([&build_dir_mount, &home_mount]).collect(),
        &base_env,
        true,
        Some(&mut stdout_handle),
        StderrTarget::Capture(&mut stderr_handle),
        script.command(),
    )?;

    Ok(())
}
