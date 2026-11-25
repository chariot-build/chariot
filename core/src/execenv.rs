use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    io::Write,
    iter,
    path::{Path, PathBuf},
    sync::Arc,
};

use chariot_rootfs::CachedPkgSet;
use chariot_runtime::{Mount, MountKind, Overlay, OverlayUpperDirectory, RuntimeError, StderrTarget};
use chariot_util::{fs::FileSystemError, hash::hash_directory};
use thiserror::Error;
use xxhash_rust::xxh3::Xxh3;

use crate::{
    CoreContext, HOST_ARCH,
    config::{
        package::{Package, PackagePlatform},
        source::Source,
    },
    workdir::WorkDirectory,
    xbps::{XBPSPackageInstallError, package_install},
};

pub const EXECENV_SOURCE_DIRECTORY_PATH: &str = "/chariot/source";
pub const EXECENV_SOURCES_DIRECTORY_PATH: &str = "/chariot/sources";
pub const EXECENV_SYSROOT_DIRECTORY_PATH: &str = "/chariot/sysroot";
pub const EXECENV_JOBSERVER_PATH: &str = "/chariot/jobserver";

#[derive(Debug, Error)]
pub enum CreateExecEnvError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error("Failed to install {} package `{}`", platform.to_string(), name)]
    PackageInstall {
        platform: PackagePlatform,
        name: String,
        source: XBPSPackageInstallError,
    },
}

pub struct ExecEnv<'a> {
    pub ctx: &'a CoreContext,
    pub pkgset: Option<Arc<CachedPkgSet>>,
    pub source: Option<Vec<PathBuf>>,
    pub sources: HashMap<String, Vec<PathBuf>>,
    pub sysroot: WorkDirectory,
    pub tool_overlay: Option<WorkDirectory>,
    pub root_readonly: bool,
    pub root_rw_overlay: Option<OverlayUpperDirectory>,
}

impl<'a> ExecEnv<'a> {
    pub fn assemble(
        ctx: &'a CoreContext,
        mut install_logger: impl FnMut() -> Box<dyn Write>,
        pkgset: Option<Arc<CachedPkgSet>>,
        source: Option<(&Source, Vec<PathBuf>)>,
        sources: &[(&Source, Vec<PathBuf>)],
        target_packages: &[(&Package, Vec<PathBuf>)],
        host_tools: &[(&Package, Vec<PathBuf>)],
        root_readonly: bool,
        root_rw_overlay: Option<OverlayUpperDirectory>,
    ) -> Result<ExecEnv<'a>, CreateExecEnvError> {
        let sysroot = WorkDirectory::create(&ctx.workdir_parent)?;
        for (pkg, paths) in target_packages {
            assert!(pkg.platform == PackagePlatform::Target);
            let mut logger = install_logger();
            package_install(
                ctx,
                None,
                &pkg.name,
                &pkg.version,
                pkg.revision,
                &pkg.global_env.target_arch,
                paths.clone(),
                &sysroot.path(),
                false,
                false,
                &mut logger,
            )
            .map_err(|err| CreateExecEnvError::PackageInstall {
                platform: PackagePlatform::Target,
                name: pkg.name.clone(),
                source: err,
            })?;
        }

        let tool_overlay = if host_tools.is_empty() {
            None
        } else {
            let tool_overlay_workdir = WorkDirectory::create(&ctx.workdir_parent)?;
            for (tool, paths) in host_tools {
                assert!(tool.platform == PackagePlatform::Host);
                let mut logger = install_logger();
                package_install(
                    ctx,
                    pkgset.as_deref(),
                    &tool.name,
                    &tool.version,
                    tool.revision,
                    HOST_ARCH,
                    paths.clone(),
                    &tool_overlay_workdir.path(),
                    true,
                    false,
                    &mut logger,
                )
                .map_err(|err| CreateExecEnvError::PackageInstall {
                    platform: PackagePlatform::Host,
                    name: tool.name.clone(),
                    source: err,
                })?;
            }
            Some(tool_overlay_workdir)
        };

        Ok(Self {
            ctx,
            pkgset,
            source: source.map(|(_, paths)| paths.clone()),
            sources: sources.iter().map(|(source, paths)| (source.name.clone(), paths.clone())).collect(),
            sysroot,
            tool_overlay,
            root_readonly,
            root_rw_overlay,
        })
    }

    pub fn compute_deps_hash(&self) -> Result<u128, FileSystemError> {
        let mut hasher = Xxh3::new();

        match &self.source {
            Some(paths) => {
                hasher.write_u8(1);
                for path in paths {
                    hash_directory(path, &mut hasher)?;
                }
            }
            None => hasher.write_u8(0),
        }

        let mut names = self.sources.keys().collect::<Vec<_>>();
        names.sort();
        for name in names {
            name.hash(&mut hasher);
            for path in &self.sources[name] {
                hash_directory(path, &mut hasher)?;
            }
        }

        hash_directory(self.sysroot.path(), &mut hasher)?;

        match &self.tool_overlay {
            Some(tool_overlay) => {
                hasher.write_u8(1);
                hash_directory(tool_overlay.path(), &mut hasher)?;
            }
            None => hasher.write_u8(0),
        }

        Ok(hasher.digest128())
    }

    pub fn exec(
        &self,
        cwd: impl AsRef<Path>,
        mounts: Vec<&Mount>,
        environment: &HashMap<impl AsRef<str>, impl AsRef<str>>,
        isolate_network: bool,
        isolate_pid: bool,
        stdin: bool,
        stdout: Option<&mut dyn Write>,
        stderr: StderrTarget<'_>,
        args: Vec<impl AsRef<str>>,
    ) -> Result<i32, RuntimeError> {
        let source_mounts = iter::chain(
            self.sources
                .iter()
                .map(|(name, paths)| (PathBuf::from(EXECENV_SOURCES_DIRECTORY_PATH).join(name), paths)),
            self.source.as_ref().map(|paths| (PathBuf::from(EXECENV_SOURCE_DIRECTORY_PATH), paths)),
        )
        .map(|(dest, paths)| Mount {
            dest,
            kind: match paths.len() {
                1 => MountKind::Bind {
                    from: paths[0].clone(),
                    read_only: true,
                    is_file: false,
                },
                _ => MountKind::OverlayFS(Overlay {
                    upper_directory: None,
                    lower_directories: paths.iter().cloned().rev().collect(),
                }),
            },
        })
        .collect::<Vec<_>>();

        let sysroot_mount = Mount {
            dest: PathBuf::from(EXECENV_SYSROOT_DIRECTORY_PATH),
            kind: MountKind::Bind {
                from: self.sysroot.path(),
                read_only: false,
                is_file: false,
            },
        };

        let jobserver_mount = Mount {
            dest: PathBuf::from(EXECENV_JOBSERVER_PATH),
            kind: MountKind::Bind {
                from: self.ctx.jobserver.path().to_path_buf(),
                read_only: false,
                is_file: true,
            },
        };

        let mountpoint_mount = Mount {
            dest: PathBuf::from("/chariot"),
            kind: MountKind::FS {
                fstype: String::from("tmpfs"),
            },
        };

        let mountpoint_readonly_remount = Mount {
            dest: PathBuf::from("/chariot"),
            kind: MountKind::Remount { readonly: true },
        };

        let mut final_mounts = vec![&mountpoint_mount];
        for source_mount in &source_mounts {
            final_mounts.push(source_mount);
        }
        final_mounts.push(&sysroot_mount);
        final_mounts.push(&jobserver_mount);
        for mount in mounts {
            final_mounts.push(mount);
        }
        final_mounts.push(&mountpoint_readonly_remount);

        let parallelism_string = self.ctx.parallelism.to_string();
        let makeflags_string = format!("--jobserver-auth=fifo:{} -j{}", EXECENV_JOBSERVER_PATH, self.ctx.jobserver.total());

        let mut base_env = HashMap::from([
            ("SOURCES_DIR", EXECENV_SOURCES_DIRECTORY_PATH),
            ("SYSROOT_DIR", EXECENV_SYSROOT_DIRECTORY_PATH),
            ("PARALLELISM", &parallelism_string),
            ("MAKEFLAGS", &makeflags_string),
        ]);

        if self.source.is_some() {
            base_env.insert("SOURCE_DIR", EXECENV_SOURCE_DIRECTORY_PATH);
        }

        self.ctx.rootfs.exec(
            cwd,
            &final_mounts,
            &base_env
                .into_iter()
                .chain(environment.iter().map(|(k, v)| (k.as_ref(), v.as_ref())))
                .collect(),
            isolate_network,
            isolate_pid,
            stdin,
            stdout,
            stderr,
            args,
            self.pkgset.as_deref(),
            !self.root_readonly,
            self.root_rw_overlay.clone(),
            match &self.tool_overlay {
                None => Vec::new(),
                Some(workdir) => vec![workdir.path()],
            },
        )
    }
}
