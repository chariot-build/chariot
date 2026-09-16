use std::{collections::HashMap, io::Write, path::PathBuf};

use chariot_runtime::{Mount, MountKind::OverlayFS, Overlay, OverlayUpperDirectory, RuntimeError};
use chariot_util::fs::FileSystemError;
use thiserror::Error;

use crate::{
    CoreContext,
    config::{
        script::Script,
        source::{Source, SourceBase},
    },
    dependencies::{ResolveDependenciesError, resolve_dependencies},
    source::{
        archive::{ArchiveFetchError, fetch_archive},
        git::{GitFetchError, fetch_git_repository},
        local::{LocalFetchError, fetch_local_source},
    },
    store::StoreEntry,
    workdir::WorkDirectory,
};

mod archive;
mod git;
mod local;

#[derive(Debug, Error)]
pub enum SourceFetchError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    ResolveDependencies(#[from] Box<ResolveDependenciesError>), // TODO: box here is nasty

    #[error(transparent)]
    Archive(#[from] ArchiveFetchError),

    #[error(transparent)]
    Git(#[from] GitFetchError),

    #[error(transparent)]
    Local(#[from] LocalFetchError),

    #[error("Patch failed")]
    Patch,

    #[error("Prepare failed")]
    Prepare,
}

pub fn fetch_source(ctx: &CoreContext, logger: &mut dyn Write, source: &Source) -> Result<Vec<StoreEntry>, SourceFetchError> {
    let mut store_entries = Vec::new();
    let (base_hash, patch_hash, prepare_hash) = source.get_hashes();

    let base_store_entry = match StoreEntry::get(&ctx.store, "source.base", base_hash)? {
        Some(store_entry) => store_entry,
        None => StoreEntry::from_workdir(
            &ctx.store,
            match &source.base {
                SourceBase::Archive(archive) => fetch_archive(ctx, logger, &archive)?,
                SourceBase::Git(git_source) => fetch_git_repository(ctx, logger, &git_source)?,
                SourceBase::Local(local_source) => fetch_local_source(ctx, logger, &local_source)?,
            },
            "source.base",
            base_hash,
        )?,
    };
    store_entries.push(base_store_entry);

    if source.patches.len() > 0 {
        let patched_store_entry = match StoreEntry::get(&ctx.store, "source.patch", patch_hash)? {
            Some(store_entry) => store_entry,
            None => {
                let overlay_work_directory = WorkDirectory::create(&ctx.workdir_parent)?;
                let work_directory = WorkDirectory::create(&ctx.workdir_parent)?;

                for patch in &source.patches {
                    let exit_code = ctx.rootfs.exec(
                        "/chariot/source",
                        &vec![&Mount {
                            dest: PathBuf::from("/chariot/source"),
                            kind: OverlayFS(Overlay {
                                lower_directories: store_entries.iter().map(|entry| entry.path()).collect(),
                                upper_directory: Some(OverlayUpperDirectory {
                                    upper_directory: work_directory.path(),
                                    work_directory: overlay_work_directory.path(),
                                }),
                            }),
                        }],
                        &HashMap::from([("CHARIOT_PATCH", patch)]),
                        logger,
                        Script::bash("echo \"$CHARIOT_PATCH\" | patch -p1").command(),
                        ctx.patch_pkgset.as_deref(),
                        None,
                    )?;

                    if exit_code != 0 {
                        return Err(SourceFetchError::Patch);
                    }
                }

                StoreEntry::from_workdir(&ctx.store, work_directory, "source.patch", patch_hash)?
            }
        };
        store_entries.push(patched_store_entry);
    };

    if let Some(prepare) = &source.prepare {
        let exec_env = resolve_dependencies(ctx, logger, &prepare.dependencies).map_err(|err| Box::new(err))?;

        let prepare_store_entry = match StoreEntry::get(&ctx.store, "source.prepare", prepare_hash)? {
            Some(store_entry) => store_entry,
            None => {
                let overlay_work_directory = WorkDirectory::create(&ctx.workdir_parent)?;
                let work_directory = WorkDirectory::create(&ctx.workdir_parent)?;

                let exit_code = exec_env.exec(
                    "/chariot/source",
                    vec![&Mount {
                        dest: PathBuf::from("/chariot/source"),
                        kind: OverlayFS(Overlay {
                            lower_directories: store_entries.iter().map(|entry| entry.path()).collect(),
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
                    logger,
                    prepare.script.command(),
                )?;

                if exit_code != 0 {
                    return Err(SourceFetchError::Prepare);
                }

                StoreEntry::from_workdir(&ctx.store, work_directory, "source.prepare", prepare_hash)?
            }
        };
        store_entries.push(prepare_store_entry);
    }

    Ok(store_entries)
}
