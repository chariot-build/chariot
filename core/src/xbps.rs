use std::{
    collections::{HashMap},
    io::Write,
    path::{Path, PathBuf},
};

use chariot_rootfs::CachedPkgSet;
use chariot_runtime::{Mount, MountKind, OverlayUpperDirectory, RuntimeError, StderrTarget};
use chariot_util::fs::FileSystemError;
use thiserror::Error;

use crate::{CoreContext, workdir::WorkDirectory};

#[derive(Debug, Error)]
pub enum XBPSValidationError {
    #[error("Package name `{}` cannot contain whitespace", .0)]
    PackageName(String),

    #[error("Package version `{}` cannot contain whitespace, `_`, or `-`", .0)]
    PackageVersion(String),

    #[error("Arch `{}` cannot contain whitespace", .0)]
    Arch(String),

    #[error("Runtime dependency `{}` cannot contain whitespace", .0)]
    RuntimeDependency(String),
}

#[derive(Debug, Error)]
pub enum XBPSPackageCreateError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    Validation(#[from] XBPSValidationError),

    #[error("xbps-create exited with non-zero exit code {}", .0)]
    CreateError(i32),

    #[error("xbps-rindex exited with non-zero exit code {}", .0)]
    RepoIndexError(i32),
}

#[derive(Debug, Error)]
pub enum XBPSPackageInstallError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    Validation(#[from] XBPSValidationError),

    #[error("xbps-install exited with non-zero exit code {}", .0)]
    Error(i32),
}

fn validate_package_name(name: &str) -> Result<(), XBPSValidationError> {
    if name.chars().any(|c| c.is_whitespace()) {
        return Err(XBPSValidationError::PackageName(name.to_string()));
    }
    Ok(())
}

fn validate_package_version(version: &str) -> Result<(), XBPSValidationError> {
    if version.chars().any(|c| c.is_whitespace() || c == '_' || c == '-') {
        return Err(XBPSValidationError::PackageVersion(version.to_string()));
    }
    Ok(())
}

fn validate_arch(arch: &str) -> Result<(), XBPSValidationError> {
    if arch.chars().any(|c| c.is_whitespace()) {
        return Err(XBPSValidationError::Arch(arch.to_string()));
    }
    Ok(())
}

pub fn package_create(
    ctx: &CoreContext,
    name: &str,
    version: &str,
    revision: usize,
    arch: &str,
    runtime_dependencies: Vec<(&str, &str, usize)>,
    from_dir: &Path,
    dest_dir: &Path,
    logger: &mut dyn Write,
) -> Result<(), XBPSPackageCreateError> {
    validate_package_name(name)?;
    validate_package_version(version)?;
    validate_arch(arch)?;

    let runtime_dependencies = runtime_dependencies
        .iter()
        .map(|(name, version, revision)| format!("{}>={}_{}", name, version, revision))
        .collect::<Vec<_>>();

    for rdep in &runtime_dependencies {
        if rdep.chars().any(|c| c.is_whitespace()) {
            return Err(XBPSPackageCreateError::Validation(XBPSValidationError::RuntimeDependency(
                rdep.to_string(),
            )));
        }
    }

    let exit_code = ctx.rootfs.exec(
        "/chariot/xbps/dest",
        &vec![
            &Mount {
                dest: PathBuf::from("/chariot"),
                kind: MountKind::FS { fstype: String::from("tmpfs") }
            },
            &Mount {
                dest: PathBuf::from("/chariot/xbps/dest"),
                kind: MountKind::Bind {
                    from: dest_dir.to_path_buf(),
                    read_only: false,
                    is_file: false,
                },
            },
            &Mount {
                dest: PathBuf::from("/chariot/xbps/package"),
                kind: MountKind::Bind {
                    from: from_dir.to_path_buf(),
                    read_only: true,
                    is_file: false,
                },
            },
        ],
        &HashMap::from([
            ("XBPS_ARCH", "invalid"),
            ("XBPS_TARGET_ARCH", arch),
            ("PKG_NAME", name),
            ("PKG_VER", version),
            ("PKG_REV", revision.to_string().as_str()),
            ("PKG_RDEPS", runtime_dependencies.join(" ").as_str())
        ]),
        false,
        true,
        false,
        Some(logger),
        StderrTarget::Merge,
        vec![
            "bash",
            "-c",
            format!("xbps-create --built-with chariot --architecture \"$XBPS_TARGET_ARCH\" --pkgver \"$PKG_NAME-${{PKG_VER}}_$PKG_REV\" --desc \"Package $PKG_NAME built by chariot\" --dependencies \"$PKG_RDEPS\" /chariot/xbps/package").as_str(),
        ],
        None,
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(XBPSPackageCreateError::CreateError(exit_code));
    }

    let exit_code = ctx.rootfs.exec(
        "/chariot/xbps/repo",
        &vec![
            &Mount {
                dest: PathBuf::from("/chariot"),
                kind: MountKind::FS {
                    fstype: String::from("tmpfs"),
                },
            },
            &Mount {
                dest: PathBuf::from("/chariot/xbps/repo"),
                kind: MountKind::Bind {
                    from: dest_dir.to_path_buf(),
                    read_only: false,
                    is_file: false,
                },
            },
        ],
        &HashMap::from([
            ("XBPS_ARCH", "invalid"),
            ("XBPS_TARGET_ARCH", arch),
            ("PKG_NAME", name),
            ("PKG_VER", version),
            ("PKG_REV", revision.to_string().as_str()),
        ]),
        false,
        true,
        false,
        Some(logger),
        StderrTarget::Merge,
        vec![
            "bash",
            "-c",
            format!("xbps-rindex -f -a \"$PKG_NAME-${{PKG_VER}}_$PKG_REV.$XBPS_TARGET_ARCH.xbps\"").as_str(),
        ],
        None,
        true,
        None,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(XBPSPackageCreateError::RepoIndexError(exit_code));
    }

    Ok(())
}

pub struct InstallablePackage<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub revision: usize,
    pub repo_dirs: Vec<PathBuf>,
}

pub fn packages_install(
    ctx: &CoreContext,
    pkgset: Option<&CachedPkgSet>,
    packages: &[InstallablePackage<'_>],
    arch: &str,
    dest_dir: &Path,
    dest_root_overlay: bool,
    force: bool,
    logger: &mut dyn Write,
) -> Result<(), XBPSPackageInstallError> {
    if packages.is_empty() {
        return Ok(());
    }

    validate_arch(arch)?;
    for pkg in packages {
        validate_package_name(pkg.name)?;
        validate_package_version(pkg.version)?;
    }

    let mountpoint_mount = Mount {
        dest: PathBuf::from("/chariot"),
        kind: MountKind::FS {
            fstype: String::from("tmpfs"),
        },
    };

    let mut repo_dirs = packages.iter().flat_map(|pkg| pkg.repo_dirs.iter().cloned()).collect::<Vec<_>>();
    repo_dirs.dedup();

    let repo_mounts = repo_dirs
        .into_iter()
        .enumerate()
        .map(|(i, repo_dir)| Mount {
            dest: PathBuf::from(format!("/chariot/xbps/repo{}", i)),
            kind: MountKind::Bind {
                from: repo_dir,
                read_only: true,
                is_file: false,
            },
        })
        .collect::<Vec<_>>();

    let dest_mount = match dest_root_overlay {
        true => None,
        false => Some(Mount {
            dest: PathBuf::from("/chariot/xbps/install"),
            kind: MountKind::Bind {
                from: dest_dir.to_path_buf(),
                read_only: false,
                is_file: false,
            },
        }),
    };

    let mut mounts = vec![&mountpoint_mount];
    for repo_mount in &repo_mounts {
        mounts.push(repo_mount);
    }
    if let Some(dest_mount) = &dest_mount {
        mounts.push(dest_mount);
    }

    let mut _workdir = None;
    let overlay = match dest_root_overlay {
        true => {
            let overlay_workdir = WorkDirectory::create(&ctx.workdir_parent)?;
            let upperdir = OverlayUpperDirectory {
                upper_directory: dest_dir.to_path_buf(),
                work_directory: overlay_workdir.path(),
            };
            _workdir = Some(overlay_workdir);
            Some(upperdir)
        }
        false => None,
    };

    let mut args = vec![
        String::from("xbps-install"),
        String::from("--reproducible"),
        String::from("--yes"),
        String::from("--ignore-file-conflicts"),
        String::from("--rootdir"),
        String::from(if dest_root_overlay { "/" } else { "/chariot/xbps/install" }),
    ];
    if force {
        args.push(String::from("--force"));
    }
    for mnt in &repo_mounts {
        args.push(String::from("--repository"));
        args.push(mnt.dest.to_string_lossy().to_string());
    }
    args.extend(packages.iter().map(|pkg| format!("{}-{}_{}", pkg.name, pkg.version, pkg.revision)));

    let exit_code = ctx.rootfs.exec(
        "/",
        &mounts,
        &HashMap::from([("XBPS_ARCH", "invalid"), ("XBPS_TARGET_ARCH", arch)]),
        false,
        true,
        false,
        Some(logger),
        StderrTarget::Merge,
        args,
        pkgset,
        overlay.is_none(),
        overlay,
        vec![],
    )?;

    if exit_code != 0 {
        return Err(XBPSPackageInstallError::Error(exit_code));
    }

    Ok(())
}

pub fn package_install(
    ctx: &CoreContext,
    pkgset: Option<&CachedPkgSet>,
    name: &str,
    version: &str,
    revision: usize,
    arch: &str,
    repo_dirs: Vec<PathBuf>,
    dest_dir: &Path,
    dest_root_overlay: bool,
    force: bool,
    logger: &mut dyn Write,
) -> Result<(), XBPSPackageInstallError> {
    packages_install(
        ctx,
        pkgset,
        &[InstallablePackage {
            name,
            version,
            revision,
            repo_dirs,
        }],
        arch,
        dest_dir,
        dest_root_overlay,
        force,
        logger,
    )
}
