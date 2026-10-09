use std::{collections::HashMap, hash::Hash, path::PathBuf};

use chariot_rootfs::CachedPkgSet;
use chariot_runtime::{
    Mount,
    MountKind::{self, OverlayFS},
    Overlay, OverlayUpperDirectory, StderrTarget,
};
use chariot_util::fs::copy_recursive;
use xxhash_rust::xxh3::Xxh3;

use crate::{
    CoreContext,
    config::{
        package::Package,
        script::Script,
        source::{Source, SourceBase},
    },
    execenv::ExecEnv,
    executor::{ExecuteError, Outcome, TaskOutput},
    graph::TaskId,
    source::{archive::fetch_archive, git::fetch_git_repository},
    store::StoreEntry,
    tracer::{PrepareStep, SourceStep, TaskStatus, Tracer},
    workdir::WorkDirectory,
};

pub(crate) mod archive;
pub(crate) mod git;

pub(crate) fn fetch(
    ctx: &CoreContext,
    tracer: &dyn Tracer,
    id: TaskId,
    source: &Source,
    sources: &[(&Source, Vec<PathBuf>)],
    target_packages: &[(&Package, Vec<PathBuf>)],
    host_tools: &[(&Package, Vec<PathBuf>)],
) -> Result<Outcome, ExecuteError> {
    tracer.task_status(id, TaskStatus::Started);

    let mut built = false;
    let mut store_entries = Vec::new();

    let base_hash = source.get_base_hash();
    let base_store_entry = match StoreEntry::get(&ctx.store, "source.base", base_hash)? {
        Some(store_entry) => store_entry,
        None => StoreEntry::from_workdir(
            &ctx.store,
            {
                built = true;

                let mut logger = tracer.source_step(id, SourceStep::FetchBase);
                match &source.base {
                    SourceBase::Archive(archive) => fetch_archive(ctx, &mut logger, archive).map_err(|err| ExecuteError::Archive {
                        source: err,
                        log: logger.captured(),
                    })?,
                    SourceBase::Git(git_source) => fetch_git_repository(ctx, &mut logger, git_source).map_err(|err| ExecuteError::Git {
                        source: err,
                        log: logger.captured(),
                    })?,
                    SourceBase::Local(local_source) => {
                        let work_dir = WorkDirectory::create(&ctx.workdir_parent)?;
                        copy_recursive(&local_source.cached_path, work_dir.path())?;
                        work_dir
                    }
                }
            },
            "source.base",
            base_hash,
        )?,
    };
    store_entries.push(base_store_entry);

    let patch_hash = source.get_patch_hash(base_hash);
    if !source.patches.is_empty() {
        let patched_store_entry = match StoreEntry::get(&ctx.store, "source.patch", patch_hash)? {
            Some(store_entry) => store_entry,
            None => {
                built = true;

                let overlay_work_directory = WorkDirectory::create(&ctx.workdir_parent)?;
                let work_directory = WorkDirectory::create(&ctx.workdir_parent)?;

                let mut logger = tracer.source_step(id, SourceStep::Patch);

                for patch in &source.patches {
                    let exit_code = ctx.rootfs.exec(
                        "/chariot/source",
                        &vec![
                            &Mount {
                                dest: PathBuf::from("/chariot"),
                                kind: MountKind::FS {
                                    fstype: String::from("tmpfs"),
                                },
                            },
                            &Mount {
                                dest: PathBuf::from("/chariot/source"),
                                kind: OverlayFS(Overlay {
                                    lower_directories: store_entries.iter().map(|entry| entry.path()).collect(),
                                    upper_directory: Some(OverlayUpperDirectory {
                                        upper_directory: work_directory.path(),
                                        work_directory: overlay_work_directory.path(),
                                    }),
                                }),
                            },
                        ],
                        &HashMap::from([("CHARIOT_PATCH", patch)]),
                        false,
                        Some(&mut logger),
                        StderrTarget::Merge,
                        Script::bash("echo \"$CHARIOT_PATCH\" | patch -p1").command(),
                        ctx.patch_pkgset.as_deref(),
                        true,
                        None,
                        vec![],
                    )?;

                    if exit_code != 0 {
                        return Err(ExecuteError::Patch { log: logger.captured() });
                    }
                }

                StoreEntry::from_workdir(&ctx.store, work_directory, "source.patch", patch_hash)?
            }
        };
        store_entries.push(patched_store_entry);
    };

    let Some(prepare) = &source.prepare else {
        return Ok(match built {
            true => Outcome::Built(TaskOutput::Source(store_entries)),
            false => Outcome::CacheHit(TaskOutput::Source(store_entries)),
        });
    };

    let prepare_hash_base = source.get_prepare_base_hash(patch_hash);
    let prepare_hash = source.get_prepare_hash(prepare_hash_base);

    let cached_entry = match ctx.ledger.lookup("source.prepare", prepare_hash)? {
        Some(effective_hash) => StoreEntry::get(&ctx.store, "source.prepare", effective_hash)?,
        None => None,
    };

    if let Some(prepared_entry) = cached_entry {
        store_entries.push(prepared_entry);
        return Ok(match built {
            true => Outcome::Built(TaskOutput::Source(store_entries)),
            false => Outcome::CacheHit(TaskOutput::Source(store_entries)),
        });
    }

    let mut pkgset_logger = tracer.prepare_step(id, PrepareStep::Pkgset);
    let root_pkgset = CachedPkgSet::get(&ctx.rootfs, &None, &prepare.global_env.global_native_packages, &mut pkgset_logger).map_err(|err| {
        ExecuteError::GetPkgSet {
            source: err,
            log: pkgset_logger.captured(),
        }
    })?;
    let pkgset =
        CachedPkgSet::get(&ctx.rootfs, &root_pkgset, &prepare.dependencies.native, &mut pkgset_logger).map_err(|err| ExecuteError::GetPkgSet {
            source: err,
            log: pkgset_logger.captured(),
        })?;

    let exec_env = ExecEnv::assemble(
        ctx,
        || tracer.prepare_step(id, PrepareStep::InstallPackage),
        pkgset,
        None,
        sources,
        target_packages,
        host_tools,
        true,
        None,
    )?;

    let effective_hash = {
        let mut hasher = Xxh3::new();
        prepare_hash_base.hash(&mut hasher);
        exec_env.compute_deps_hash()?.hash(&mut hasher);
        hasher.digest128()
    };

    let entry = match StoreEntry::get(&ctx.store, "source.prepare", effective_hash)? {
        Some(store_entry) => store_entry,
        None => {
            let overlay_work_directory = WorkDirectory::create(&ctx.workdir_parent)?;
            let work_directory = WorkDirectory::create(&ctx.workdir_parent)?;

            let mut logger = tracer.prepare_step(id, PrepareStep::Prepare);
            let exit_code = exec_env.exec(
                "/chariot/source",
                vec![&Mount {
                    dest: PathBuf::from("/chariot/source"),
                    kind: OverlayFS(Overlay {
                        lower_directories: store_entries.iter().rev().map(StoreEntry::path).collect(),
                        upper_directory: Some(OverlayUpperDirectory {
                            upper_directory: work_directory.path(),
                            work_directory: overlay_work_directory.path(),
                        }),
                    }),
                }],
                &prepare
                    .global_env
                    .global_environment_variables
                    .iter()
                    .chain(&prepare.environment_variables)
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .chain([("SOURCE_DIR", "/chariot/source")])
                    .collect(),
                false,
                Some(&mut logger),
                StderrTarget::Merge,
                prepare.script.command(),
            )?;

            if exit_code != 0 {
                return Err(ExecuteError::Prepare { log: logger.captured() });
            }

            StoreEntry::from_workdir(&ctx.store, work_directory, "source.prepare", effective_hash)?
        }
    };

    ctx.ledger.record("source.prepare", prepare_hash, effective_hash)?;

    store_entries.push(entry);
    Ok(Outcome::Built(TaskOutput::Source(store_entries)))
}
