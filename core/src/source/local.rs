use std::{fs, io::Write, path::PathBuf};

use chariot_util::fs::{FileSystemError, copy_recursive};
use thiserror::Error;

use crate::{
    CoreContext,
    cache::WorkDirectory,
    config::source::LocalSource,
};

#[derive(Debug, Error)]
pub enum LocalFetchError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
}

pub fn fetch_local_source(ctx: &CoreContext, _logger: &mut dyn Write, local_source: &LocalSource) -> Result<WorkDirectory, LocalFetchError> {
    let work_directory = WorkDirectory::create(&ctx.cache)?;

    let source_path = PathBuf::from(&local_source.path);
    let source_meta = fs::metadata(&source_path).map_err(|err| FileSystemError::Metadata {
        path: source_path.clone(),
        source: err,
    })?;

    if source_meta.is_dir() {
        copy_recursive(&source_path, work_directory.path())?;
    } else if let Some(file_name) = source_path.file_name() {
        let file_dest = work_directory.path().join(file_name);
        fs::copy(&source_path, &file_dest).map_err(|err| FileSystemError::CopyFile {
            from: source_path,
            to: file_dest,
            source: err,
        })?;
    }

    Ok(work_directory)
}