use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use chariot_rootfs::{CachedPkgSet, RootFS};
use chariot_runtime::{Mount, MountKind, RuntimeError, StderrTarget};
use chariot_util::fs::FileSystemError;
use thiserror::Error;

use crate::{
    CoreContext,
    config::{
        script::Script,
        source::{Archive, ArchiveKind},
    },
    workdir::WorkDirectory,
};

#[derive(Debug, Error)]
pub enum ArchiveFetchError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error("Failed to download source archive")]
    Download,

    #[error("Checksum validation for archive failed")]
    ChecksumMismatch,

    #[error("Failed to extract source archive")]
    Extract,
}

pub fn fetch_archive(ctx: &CoreContext, logger: &mut dyn Write, archive: &Archive) -> Result<WorkDirectory, ArchiveFetchError> {
    const ARCHIVE_NAME: &str = "archive";

    let download_dir = WorkDirectory::create(&ctx.workdir_parent)?;
    let archive_path = download_dir.path().join(ARCHIVE_NAME);

    download_archive(
        &ctx.rootfs,
        ctx.wget_pkgset.as_deref(),
        ctx.sha256sum_pkgset.as_deref(),
        logger,
        &archive.url,
        &archive.checksum,
        &download_dir.path(),
        ARCHIVE_NAME,
    )?;

    let work_directory = WorkDirectory::create(&ctx.workdir_parent)?;
    extract_archive(
        &ctx.rootfs,
        ctx.bsdtar_pkgset.as_deref(),
        ctx.unzip_pkgset.as_deref(),
        logger,
        &archive.kind,
        &archive_path,
        &work_directory.path(),
    )?;

    Ok(work_directory)
}

pub fn download_archive(
    rootfs: &Arc<RootFS>,
    wget_pkgset: Option<&CachedPkgSet>,
    sha256sum_pkgset: Option<&CachedPkgSet>,
    logger: &mut dyn Write,
    url: &str,
    checksum: &str,
    dest_dir: &Path,
    dest_filename: &str,
) -> Result<(), ArchiveFetchError> {
    let download_dir_bind = Mount {
        dest: PathBuf::from("/chariot"),
        kind: MountKind::Bind {
            from: dest_dir.to_path_buf(),
            read_only: false,
            is_file: false,
        },
    };

    let archive_path = PathBuf::from("/chariot").join(dest_filename);

    let exit_code = rootfs.exec(
        "/",
        &vec![&download_dir_bind],
        &HashMap::from([("ARCHIVE_PATH", archive_path.to_string_lossy().as_ref()), ("ARCHIVE_URL", url)]),
        false,
        Some(logger),
        StderrTarget::Merge,
        Script::bash("wget --no-hsts -q -O \"$ARCHIVE_PATH\" \"$ARCHIVE_URL\"").command(),
        wget_pkgset,
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(ArchiveFetchError::Download);
    }

    let exit_code = rootfs.exec(
        "/",
        &vec![&download_dir_bind],
        &HashMap::from([("ARCHIVE_PATH", archive_path.to_string_lossy().as_ref()), ("ARCHIVE_CHECKSUM", checksum)]),
        false,
        Some(logger),
        StderrTarget::Merge,
        Script::bash("echo \"$ARCHIVE_CHECKSUM  $ARCHIVE_PATH\n\" | sha256sum -c -").command(),
        sha256sum_pkgset,
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(ArchiveFetchError::ChecksumMismatch);
    }

    Ok(())
}

pub fn extract_archive(
    rootfs: &Arc<RootFS>,
    bsdtar_pkgset: Option<&CachedPkgSet>,
    unzip_pkgset: Option<&CachedPkgSet>,
    logger: &mut dyn Write,
    kind: &ArchiveKind,
    src: &Path,
    dest: &Path,
) -> Result<(), ArchiveFetchError> {
    let (pkgset, script) = match kind {
        ArchiveKind::Tar => (
            bsdtar_pkgset,
            Script::bash("bsdtar --no-same-owner --strip-components 1 -x -C /chariot/dest -f /chariot/source"),
        ),
        ArchiveKind::Zip => (unzip_pkgset, Script::bash("unzip -o /chariot/source -d /chariot/dest")),
    };

    let exit_code = rootfs.exec(
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
                kind: MountKind::Bind {
                    from: src.to_path_buf(),
                    read_only: true,
                    is_file: true,
                },
            },
            &Mount {
                dest: PathBuf::from("/chariot/dest"),
                kind: MountKind::Bind {
                    from: dest.to_path_buf(),
                    read_only: false,
                    is_file: false,
                },
            },
        ],
        &HashMap::<&str, &str>::new(),
        false,
        Some(logger),
        StderrTarget::Merge,
        script.command(),
        pkgset,
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(ArchiveFetchError::Extract);
    }

    Ok(())
}
