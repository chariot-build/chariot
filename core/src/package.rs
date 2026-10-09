use std::{hash::Hash, path::PathBuf};

use chariot_rootfs::CachedPkgSet;
use chariot_runtime::{Mount, MountKind, StderrTarget};
use chariot_util::{fs::force_rm_contents, hash::hash_directory};
use xxhash_rust::xxh3::Xxh3;

use crate::{
    CoreContext,
    buildcache::BuildDirectory,
    config::{package::Package, source::Source},
    execenv::ExecEnv,
    executor::{ExecuteError, Outcome, TaskOutput},
    graph::TaskId,
    store::StoreEntry,
    tracer::{PackageStep, TaskStatus, Tracer},
    workdir::WorkDirectory,
    xbps::package_create,
};

pub const PACKAGE_BUILD_DIR: &str = "/chariot/build";

pub(crate) fn build(
    ctx: &CoreContext,
    tracer: &dyn Tracer,
    id: TaskId,
    package: &Package,
    source: Option<(&Source, Vec<PathBuf>)>,
    sources: &[(&Source, Vec<PathBuf>)],
    target_packages: &[(&Package, Vec<PathBuf>)],
    host_tools: &[(&Package, Vec<PathBuf>)],
) -> Result<Outcome, ExecuteError> {
    let pkg_hash = package.get_package_hash();

    if let Some(effective_hash) = ctx.ledger.lookup("pkg", pkg_hash)?
        && let Some(store_entry) = StoreEntry::get(&ctx.store, "pkg", effective_hash)?
    {
        return Ok(Outcome::CacheHit(TaskOutput::Package(store_entry)));
    }

    tracer.task_status(id, TaskStatus::Started);

    let install_store_entry = 'install: {
        let pkg_content_hash = package.get_content_hash();

        if let Some(effective_hash) = ctx.ledger.lookup("install", pkg_content_hash)?
            && let Some(store_entry) = StoreEntry::get(&ctx.store, "install", effective_hash)?
        {
            break 'install store_entry;
        }

        let mut pkgset_logger = tracer.package_step(id, PackageStep::Pkgset);
        let root_pkgset = CachedPkgSet::get(&ctx.rootfs, &None, &package.global_env.global_native_packages, &mut pkgset_logger).map_err(|err| {
            ExecuteError::GetPkgSet {
                source: err,
                log: pkgset_logger.captured(),
            }
        })?;
        let pkgset = CachedPkgSet::get(&ctx.rootfs, &root_pkgset, &package.dependencies.native, &mut pkgset_logger).map_err(|err| {
            ExecuteError::GetPkgSet {
                source: err,
                log: pkgset_logger.captured(),
            }
        })?;

        let exec_env = ExecEnv::assemble(
            ctx,
            || tracer.package_step(id, PackageStep::InstallPackage),
            pkgset,
            source,
            sources,
            target_packages,
            host_tools,
            true,
            None,
        )?;

        let effective_hash = {
            let mut hasher = Xxh3::new();
            package.get_content_base_hash().hash(&mut hasher);
            exec_env.compute_deps_hash()?.hash(&mut hasher);
            hasher.digest128()
        };

        if let Some(store_entry) = StoreEntry::get(&ctx.store, "install", effective_hash)? {
            ctx.ledger.record("install", pkg_content_hash, effective_hash)?;
            break 'install store_entry;
        }

        let build_cachedir = BuildDirectory::get_rw(&ctx.build_cache, package.platform, package.get_arch(), &package.name)?;
        if !ctx
            .build_cache_enabled
            .iter()
            .any(|(platform, name)| platform == &package.platform && name == &package.name)
        {
            force_rm_contents(build_cachedir.path(), None)?;
        };

        let build_mount = Mount {
            dest: PathBuf::from(PACKAGE_BUILD_DIR),
            kind: MountKind::Bind {
                from: build_cachedir.path(),
                read_only: false,
                is_file: false,
            },
        };

        let base_env = package
            .global_env
            .global_environment_variables
            .iter()
            .chain(&package.environment_variables)
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .chain([
                ("BUILD_DIR", PACKAGE_BUILD_DIR),
                ("PREFIX", package.get_prefix()),
                ("ARCH", package.get_arch()),
            ])
            .collect();

        if let Some(configure) = &package.configure {
            let mut logger = tracer.package_step(id, PackageStep::Configure);
            let exit_code = exec_env.exec(
                PACKAGE_BUILD_DIR,
                vec![&build_mount],
                &base_env,
                false,
                Some(&mut logger),
                StderrTarget::Merge,
                configure.command(),
            )?;

            if exit_code != 0 {
                return Err(ExecuteError::Configure {
                    exit_code,
                    log: logger.captured(),
                });
            }
        }

        if let Some(build) = &package.build {
            let mut logger = tracer.package_step(id, PackageStep::Build);
            let exit_code = exec_env.exec(
                PACKAGE_BUILD_DIR,
                vec![&build_mount],
                &base_env,
                false,
                Some(&mut logger),
                StderrTarget::Merge,
                build.command(),
            )?;

            if exit_code != 0 {
                return Err(ExecuteError::Build {
                    exit_code,
                    log: logger.captured(),
                });
            }
        }

        let install_workdir = WorkDirectory::create(&ctx.workdir_parent)?;

        let mut logger = tracer.package_step(id, PackageStep::Install);
        let exit_code = exec_env.exec(
            PACKAGE_BUILD_DIR,
            vec![
                &build_mount,
                &Mount {
                    dest: PathBuf::from("/chariot/install"),
                    kind: MountKind::Bind {
                        from: install_workdir.path(),
                        read_only: false,
                        is_file: false,
                    },
                },
            ],
            &base_env.into_iter().chain([("INSTALL_DIR", "/chariot/install")]).collect(),
            false,
            Some(&mut logger),
            StderrTarget::Merge,
            package.install.command(),
        )?;

        if exit_code != 0 {
            return Err(ExecuteError::Install {
                exit_code,
                log: logger.captured(),
            });
        }

        let store_entry = StoreEntry::from_workdir(&ctx.store, install_workdir, "install", effective_hash)?;
        ctx.ledger.record("install", pkg_content_hash, effective_hash)?;

        store_entry
    };

    let effective_hash = {
        let mut hasher = Xxh3::new();
        package.get_package_meta_hash().hash(&mut hasher);
        hash_directory(install_store_entry.path(), &mut hasher)?;
        hasher.digest128()
    };

    if let Some(store_entry) = StoreEntry::get(&ctx.store, "pkg", effective_hash)? {
        ctx.ledger.record("pkg", pkg_hash, effective_hash)?;
        return Ok(Outcome::Built(TaskOutput::Package(store_entry)));
    }

    let workdir = WorkDirectory::create(&ctx.workdir_parent)?;
    let mut logger = tracer.package_step(id, PackageStep::Package);
    package_create(
        ctx,
        &package.name,
        &package.version,
        package.revision,
        package.get_arch(),
        package
            .runtime_dependencies
            .iter()
            .map(|pkg| (pkg.name.as_str(), pkg.version.as_str(), pkg.revision))
            .collect(),
        &install_store_entry.path(),
        &workdir.path(),
        &mut logger,
    )
    .map_err(|err| ExecuteError::PackageCreate {
        source: err,
        log: logger.captured(),
    })?;

    let store_entry = StoreEntry::from_workdir(&ctx.store, workdir, "pkg", effective_hash)?;
    ctx.ledger.record("pkg", pkg_hash, effective_hash)?;

    Ok(Outcome::Built(TaskOutput::Package(store_entry)))
}
