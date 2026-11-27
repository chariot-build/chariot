use std::{collections::HashMap, io::Write, path::PathBuf};

use chariot_runtime::{Mount, MountKind, RuntimeError, StderrTarget};
use chariot_util::fs::FileSystemError;
use thiserror::Error;

use crate::{CoreContext, bash_command, config::source::GitSource, workdir::WorkDirectory};

#[derive(Debug, Error)]
pub enum GitFetchError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error("Git ls-remote failed")]
    LsRemote,

    #[error("Git clone failed")]
    Clone,

    #[error("Git fetch failed")]
    Fetch,

    #[error("Git revision is a HEAD (branch)")]
    RefIsHead,

    #[error("Git checkout failed for source")]
    Checkout,
}

pub fn fetch_git_repository(ctx: &CoreContext, logger: &mut dyn Write, git_source: &GitSource) -> Result<WorkDirectory, GitFetchError> {
    if git_source.revision.is_empty() {
        return Err(GitFetchError::RefIsHead);
    }

    let mountpoint_bind = Mount {
        dest: PathBuf::from("/chariot"),
        kind: MountKind::FS {
            fstype: String::from("tmpfs"),
        },
    };

    let work_directory = WorkDirectory::create(&ctx.workdir_parent)?;
    let source_bind = Mount {
        dest: PathBuf::from("/chariot/source"),
        kind: MountKind::Bind {
            from: work_directory.path(),
            read_only: false,
            is_file: false,
        },
    };

    let mounts = vec![&mountpoint_bind, &source_bind];

    let exit_code = ctx.rootfs.exec(
        "/chariot/source",
        &mounts,
        &HashMap::from([("GIT_URL", git_source.url.as_str()), ("GIT_REV", git_source.revision.as_str())]),
        false,
        true,
        false,
        Some(logger),
        StderrTarget::Merge,
        bash_command(
            r#"
            REMOTE_REF=$(git ls-remote "$GIT_URL" "refs/heads/$GIT_REV") || exit 1
            if [[ -n "$REMOTE_REF" ]]; then
                exit 67
            fi
        "#,
        ),
        ctx.git_pkgset.as_deref(),
        true,
        None,
        vec![],
    )?;

    if exit_code == 67 {
        return Err(GitFetchError::RefIsHead);
    }

    if exit_code != 0 {
        return Err(GitFetchError::LsRemote);
    }

    let exit_code = ctx.rootfs.exec(
        "/chariot/source",
        &mounts,
        &HashMap::from([("GIT_URL", git_source.url.as_str())]),
        false,
        true,
        false,
        Some(logger),
        StderrTarget::Merge,
        bash_command("git clone --depth=1 \"$GIT_URL\" ."),
        ctx.git_pkgset.as_deref(),
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(GitFetchError::Clone);
    }

    let exit_code = ctx.rootfs.exec(
        "/chariot/source",
        &mounts,
        &HashMap::from([("GIT_REV", git_source.revision.as_str())]),
        false,
        true,
        false,
        Some(logger),
        StderrTarget::Merge,
        bash_command("git fetch --depth=1 origin \"$GIT_REV\""),
        ctx.git_pkgset.as_deref(),
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(GitFetchError::Fetch);
    }

    let exit_code = ctx.rootfs.exec(
        "/chariot/source",
        &mounts,
        &HashMap::<&str, &str>::new(),
        false,
        true,
        false,
        Some(logger),
        StderrTarget::Merge,
        bash_command("git checkout FETCH_HEAD"),
        ctx.git_pkgset.as_deref(),
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(GitFetchError::Checkout);
    }

    Ok(work_directory)
}
