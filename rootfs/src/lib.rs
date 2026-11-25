use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{File, canonicalize, write},
    io::{self, Cursor, ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use chariot_runtime::{Mount, MountKind, Overlay, OverlayUpperDirectory, RuntimeError, runtime_execute};
use chariot_util::{
    fs::{FileSystemError, MergeDirectoryError, force_rm, force_rm_contents, join_soft, make_path, merge_directory},
    lock::{DirLock, LockShared, block_attempted},
};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use tar::Archive;
use thiserror::Error;
use xz2::read::XzDecoder;

use crate::{
    db::Database,
    manifest::{Manifest, ManifestFetchError, PLACEHOLDER_PACKAGE, PLACEHOLDER_ROOT_PACKAGES},
    state::{CachedManifest, State},
};

pub use chariot_runtime::StderrTarget;
pub use manifest::ManifestFetchSpec;
pub use pkgset::{CachedPkgSet, GetPkgSetError, PkgSetState};
pub use state::{StateReadError, StateWriteError};

mod db;
mod manifest;
mod pkgset;
mod state;

pub const DEFAULT_MANIFESTS_URL: &str = "https://cdn.chariot-build.dev/manifests/@ARCH@/@VERSION@.toml";

const ROOT_UID: u32 = 0;
const ROOT_GID: u32 = 0;

pub const CHARIOT_USER_UID: u32 = 1000;
pub const CHARIOT_USER_GID: u32 = 1000;
pub const CHARIOT_USER_NAME: &str = "chariot";
pub const CHARIOT_USER_GROUP: &str = "chariot";

/// Describes the version of the on-disk rootfs. If the on-disk representation
/// changes in a backwards incompatible way, this version should be bumped.
const ROOTFS_VERSION: i64 = 4;

#[derive(Debug, Error)]
pub enum RootFSInitError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    Database(#[from] rusqlite::Error),

    #[error("Failed to write state file")]
    StateWrite(#[from] StateWriteError),

    #[error("RootFS manifest fetch error")]
    ManifestFetch(#[from] ManifestFetchError),

    #[error("RootFS setup command exited with a non-zero code")]
    SetupScript,

    #[error("Failed to install archive `{}`", url)]
    ArchiveInstall { url: String, source: ArchiveInstallError },

    #[error("Invalid path `{}`", path.display())]
    InvalidPath { path: PathBuf, source: io::Error },
}

#[derive(Debug, Error)]
pub enum ArchiveInstallError {
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    MergeDirectory(#[from] MergeDirectoryError),

    #[error("Failed to unpack tar archive to `{}`", to.display())]
    TarUnpack { to: PathBuf, source: io::Error },

    #[error("Hash does not match expected hash, expected `{}`, got `{}`", expected, found)]
    HashMismatch { expected: String, found: String },

    #[error("Unsupported compression requested `{}`", .0)]
    UnknownCompression(String),

    #[error("Zstd decompression error")]
    ZstdDecompression(#[source] io::Error),
}

#[derive(Debug, Error)]
pub enum RootFSGetError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    Database(#[from] rusqlite::Error),

    #[error("Failed to read state file")]
    StateRead(#[from] StateReadError),

    #[error("Invalid path `{}`", path.display())]
    InvalidPath { path: PathBuf, source: io::Error },
}

#[derive(Debug, Error)]
pub enum RootFSPruneError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum RootFSListError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error(transparent)]
    Database(#[from] rusqlite::Error),
}

pub struct RootFS {
    _lock: DirLock<LockShared>,
    path: PathBuf,
    state: State,
    db: Mutex<Database>,
}

enum RootFSPath {
    Gitignore,
    State,
    Database,
    Fs,
    ArchiveTmp,
    PackageSets,
    PackageSet(i64),
    PackageSetWork,
}

pub struct PkgSetMeta {
    pub id: i64,
    pub state: PkgSetState,
    pub base: Option<i64>,
    pub base_depth: u64,
    pub size: u64,
    pub packages: HashSet<String>,
}

fn rootfs_sub_path(rootfs_path: impl AsRef<Path>, sub_path: RootFSPath) -> PathBuf {
    let base = rootfs_path.as_ref();
    match sub_path {
        RootFSPath::Gitignore => base.join(".gitignore"),
        RootFSPath::State => base.join("state.toml"),
        RootFSPath::Database => base.join("rootfsdb.sqlite"),
        RootFSPath::Fs => base.join("fs"),
        RootFSPath::ArchiveTmp => base.join(".archive_tmp"),
        RootFSPath::PackageSets => base.join("pkgsets"),
        RootFSPath::PackageSet(id) => base.join("pkgsets").join(id.to_string()),
        RootFSPath::PackageSetWork => base.join("pkgsets").join(".work"),
    }
}

impl RootFS {
    pub fn init(path: impl AsRef<Path>, manifest_spec: &ManifestFetchSpec, logger: &mut dyn Write) -> Result<Self, RootFSInitError> {
        make_path(&path)?;

        let path = canonicalize(&path).map_err(|err| RootFSInitError::InvalidPath {
            path: path.as_ref().to_path_buf(),
            source: err,
        })?;

        let manifest = Manifest::fetch(&manifest_spec)?;

        let rootfs_lock = DirLock::exclusive_noblock(&path)?;
        force_rm_contents(&path, None)?;

        let gitignore_path = rootfs_sub_path(&path, RootFSPath::Gitignore);
        write(&gitignore_path, "# Generated by Chariot\n*").map_err(|err| FileSystemError::WriteFile {
            path: gitignore_path.clone(),
            source: err,
        })?;

        for sub_path in [RootFSPath::Fs, RootFSPath::PackageSets] {
            make_path(rootfs_sub_path(&path, sub_path))?;
        }

        let state = State {
            manifest: manifest_spec.clone(),
            cached_manifest: CachedManifest {
                root_packages: manifest.packages.root,
                binary_to_package_map: manifest.packages.binary_map,
                command_pkg_download: manifest.commands.pkg_download,
                command_pkg_install: manifest.commands.pkg_install,
            },
        };

        state.write(rootfs_sub_path(&path, RootFSPath::State), false)?;

        for directory in manifest.directories {
            make_path(join_soft(rootfs_sub_path(&path, RootFSPath::Fs), directory))?;
        }

        for file in manifest.files {
            let path = join_soft(rootfs_sub_path(&path, RootFSPath::Fs), file);
            if let Some(parent_path) = path.parent() {
                make_path(parent_path)?;
                File::create(&path).map_err(|err| FileSystemError::CreateFile {
                    path: path.to_path_buf(),
                    source: err,
                })?;
            }
        }

        for archive in manifest.archives {
            Self::install_archive(
                &rootfs_sub_path(&path, RootFSPath::Fs),
                &rootfs_sub_path(&path, RootFSPath::ArchiveTmp),
                &archive.url,
                &archive.compression,
                &archive.hash,
                archive.subdir.as_deref(),
            )
            .map_err(|err| RootFSInitError::ArchiveInstall {
                url: archive.url,
                source: err,
            })?;
        }

        let setup_command = manifest.commands.setup.replace(
            PLACEHOLDER_ROOT_PACKAGES,
            &state
                .cached_manifest
                .root_packages
                .iter()
                .map(|str| str.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );

        let exit_code = runtime_execute(
            rootfs_sub_path(&path, RootFSPath::Fs),
            false,
            ROOT_UID,
            ROOT_GID,
            "/",
            &vec![],
            &vec![],
            &HashMap::<&str, &str>::new(),
            false,
            true,
            false,
            Some(logger),
            StderrTarget::Merge,
            vec!["bash", "-c", &setup_command],
        )?;

        if exit_code != 0 {
            return Err(RootFSInitError::SetupScript);
        }

        let db = Database::connect(rootfs_sub_path(&path, RootFSPath::Database))?;

        state.write(rootfs_sub_path(&path, RootFSPath::State), true)?;

        Ok(Self {
            _lock: rootfs_lock.relock_shared_noblock()?,
            path,
            state,
            db: Mutex::new(db),
        })
    }

    pub fn get(path: impl AsRef<Path>) -> Result<Option<Self>, RootFSGetError> {
        let path = match canonicalize(&path) {
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(RootFSGetError::InvalidPath {
                    path: path.as_ref().to_path_buf(),
                    source: err,
                });
            }
            Ok(path) => path,
        };

        let rootfs_lock = match DirLock::shared(&path) {
            Err(FileSystemError::Open { source, .. }) if source.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
            Ok(lock) => lock,
        };

        let state = match State::read(rootfs_sub_path(&path, RootFSPath::State))? {
            None => return Ok(None),
            Some(state) => state,
        };

        Ok(Some(Self {
            _lock: rootfs_lock,
            db: Mutex::new(Database::connect(rootfs_sub_path(&path, RootFSPath::Database))?),
            path,
            state,
        }))
    }

    fn install_archive(
        dest: &Path,
        tmp_path: &Path,
        url: &str,
        compression: &str,
        hash: &str,
        subdir: Option<&str>,
    ) -> Result<(), ArchiveInstallError> {
        let client = Client::builder().timeout(None).connect_timeout(Duration::from_secs(30)).build()?;
        let archive_data = client.get(url).send()?.error_for_status()?.bytes()?;
        let archive_hash = {
            let mut hasher = Sha256::new();
            hasher.update(&archive_data);
            hex::encode(hasher.finalize())
        };

        if archive_hash != hash.as_ref() {
            return Err(ArchiveInstallError::HashMismatch {
                expected: hash.to_string(),
                found: archive_hash,
            });
        }

        let decompressor: &mut dyn io::Read = match compression.as_ref() {
            "xz" => &mut XzDecoder::new(Cursor::new(archive_data)),
            "zstd" => &mut zstd::Decoder::new(Cursor::new(archive_data)).map_err(|err| ArchiveInstallError::ZstdDecompression(err))?,
            _ => return Err(ArchiveInstallError::UnknownCompression(compression.to_string())),
        };

        let unpack_path = match subdir {
            None => dest,
            Some(_) => tmp_path,
        };

        Archive::new(decompressor)
            .unpack(&unpack_path)
            .map_err(|err| ArchiveInstallError::TarUnpack {
                to: unpack_path.to_path_buf(),
                source: err,
            })?;

        if let Some(subdir) = subdir {
            let from_path = unpack_path.join(subdir);
            merge_directory(&from_path, &dest)?;
            force_rm(unpack_path)?;
        }

        Ok(())
    }

    fn download_native_package(&self, package: impl AsRef<str>, logger: &mut dyn Write) -> Result<bool, RuntimeError> {
        let exit_code = runtime_execute(
            self.sub_path(RootFSPath::Fs),
            false,
            ROOT_UID,
            ROOT_GID,
            "/",
            &vec![],
            &vec![],
            &HashMap::<&str, &str>::new(),
            false,
            true,
            false,
            Some(logger),
            StderrTarget::Merge,
            vec![
                "bash",
                "-c",
                &self
                    .state
                    .cached_manifest
                    .command_pkg_download
                    .replace(PLACEHOLDER_PACKAGE, package.as_ref()),
            ],
        )?;

        Ok(exit_code == 0)
    }

    fn install_native_package(
        &self,
        base: Option<&CachedPkgSet>,
        install_path: impl AsRef<Path>,
        work_path: impl AsRef<Path>,
        package: impl AsRef<str>,
        logger: &mut dyn Write,
    ) -> Result<bool, RuntimeError> {
        let mut lower_directories = vec![self.sub_path(RootFSPath::Fs)];

        let mut base = base;
        while let Some(pkgset) = base {
            lower_directories.insert(1, pkgset.path());
            base = pkgset.base.as_deref();
        }

        lower_directories.reverse();

        let exit_code = runtime_execute(
            self.sub_path(RootFSPath::Fs),
            false,
            ROOT_UID,
            ROOT_GID,
            "/",
            &vec![&Mount {
                dest: PathBuf::new(),
                kind: MountKind::OverlayFS(Overlay {
                    upper_directory: Some(OverlayUpperDirectory {
                        upper_directory: install_path.as_ref().to_path_buf(),
                        work_directory: work_path.as_ref().to_path_buf(),
                    }),
                    lower_directories,
                }),
            }],
            &vec![],
            &HashMap::<&str, &str>::new(),
            false,
            true,
            false,
            Some(logger),
            StderrTarget::Merge,
            vec![
                "bash",
                "-c",
                &self
                    .state
                    .cached_manifest
                    .command_pkg_install
                    .replace(PLACEHOLDER_PACKAGE, package.as_ref()),
            ],
        )?;

        return Ok(exit_code == 0);
    }

    pub fn list_pkgsets(&self) -> Result<Vec<PkgSetMeta>, RootFSListError> {
        let _pkgsets_lock = DirLock::exclusive(self.sub_path(RootFSPath::PackageSets))?;

        let pkgsets = self.db.lock().unwrap().get_pkgsets()?;

        Ok(pkgsets)
    }

    pub fn prune_pkgsets(&self, predicate: fn(pkgset_meta: &PkgSetMeta) -> bool) -> Result<(usize, usize, usize), RootFSPruneError> {
        let _pkgsets_lock = DirLock::exclusive(self.sub_path(RootFSPath::PackageSets))?;

        let pkgsets = self
            .db
            .lock()
            .unwrap()
            .get_pkgsets()?
            .into_iter()
            .map(|meta| (meta.id, meta))
            .collect::<HashMap<_, _>>();

        let pkgsets_total = pkgsets.len();
        let mut pkgsets_removed: usize = 0;
        let mut pkgsets_in_use: usize = 0;

        let mut dep_counts = HashMap::new();
        for meta in pkgsets.values() {
            if let Some(base) = meta.base {
                *dep_counts.entry(base).or_insert(0) += 1;
            }
        }

        let mut queued_pkgsets = VecDeque::new();
        for id in pkgsets.keys() {
            if dep_counts.get(id).copied().unwrap_or(0) > 0 {
                continue;
            }
            queued_pkgsets.push_back(*id);
        }

        while let Some(pkgset_id) = queued_pkgsets.pop_front() {
            let pkgset = &pkgsets[&pkgset_id];
            let path = self.sub_path(RootFSPath::PackageSet(pkgset.id));

            let _lock = match DirLock::exclusive_noblock(&path) {
                result if block_attempted(&result) => {
                    pkgsets_in_use += 1;
                    continue;
                }
                result => result,
            }?;

            if !predicate(&pkgset) {
                continue;
            }

            self.db.lock().unwrap().update_pkgset(pkgset.id, &PkgSetState::Unknown, None)?;
            force_rm(&path)?;

            self.db.lock().unwrap().remove_pkgset(pkgset.id)?;
            pkgsets_removed += 1;

            if let Some(base) = &pkgset.base {
                if let Some(count) = dep_counts.get_mut(base) {
                    *count -= 1;
                    if *count == 0 {
                        dep_counts.remove(base);
                        queued_pkgsets.push_back(*base);
                    }
                }
            }
        }

        Ok((pkgsets_total, pkgsets_removed, pkgsets_in_use))
    }

    fn sub_path(&self, sub_path: RootFSPath) -> PathBuf {
        rootfs_sub_path(&self.path, sub_path)
    }

    pub fn get_manifest_spec(&self) -> &ManifestFetchSpec {
        &self.state.manifest
    }

    pub fn lookup_package_of_binary(&self, binary: impl AsRef<str>) -> Option<&String> {
        self.state.cached_manifest.binary_to_package_map.get(binary.as_ref())
    }

    pub fn exec(
        self: &Arc<Self>,
        cwd: impl AsRef<Path>,
        mounts: &Vec<&Mount>,
        environment: &HashMap<impl AsRef<str>, impl AsRef<str>>,
        isolate_network: bool,
        isolate_pid: bool,
        stdin: bool,
        stdout: Option<&mut dyn Write>,
        stderr: StderrTarget<'_>,
        args: Vec<impl AsRef<str>>,
        pkgset: Option<&CachedPkgSet>,
        root_readonly: bool,
        root_rw_overlay: Option<OverlayUpperDirectory>,
        root_overlays: Vec<PathBuf>,
    ) -> Result<i32, RuntimeError> {
        let mut early_mounts = Vec::new();

        let mut lower_directories = root_overlays;
        if let Some(pkgset) = pkgset {
            assert!(pkgset.rootfs.path == self.path);

            lower_directories.push(pkgset.path());

            let mut base = &pkgset.base;
            while let Some(pkgset) = base {
                lower_directories.push(pkgset.path());
                base = &pkgset.base;
            }
        }

        if root_rw_overlay.is_some() || lower_directories.len() > 0 {
            lower_directories.push(self.sub_path(RootFSPath::Fs));

            early_mounts.push(Mount {
                dest: PathBuf::new(),
                kind: MountKind::OverlayFS(Overlay {
                    upper_directory: root_rw_overlay,
                    lower_directories,
                }),
            });
        }

        runtime_execute(
            self.sub_path(RootFSPath::Fs),
            root_readonly,
            CHARIOT_USER_UID,
            CHARIOT_USER_GID,
            cwd,
            &early_mounts.iter().collect(),
            mounts,
            environment,
            isolate_network,
            isolate_pid,
            stdin,
            stdout,
            stderr,
            args,
        )
    }
}
