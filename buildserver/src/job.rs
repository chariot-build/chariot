use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::stdout,
    iter,
    num::NonZero,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result, bail};
use chariot_config::{DEFAULT_BASE_CONFIG_PATH, DEFAULT_LUA_CONFIG_PATH, base::read_base_config, lua::eval_lua_config};
use chariot_core::{
    CoreContext, DEFAULT_TARGET_PREFIX,
    buildcache::BuildCache,
    config::{GlobalEnvironment, package::PackagePlatform},
    executor::{BuildManager, FailureMode},
    graph::BuildGraphBuilder,
    store::Store,
    workdir::{WorkDirectory, WorkDirectoryParent},
};
use chariot_rootfs::{CachedPkgSet, DEFAULT_MANIFESTS_URL, ManifestFetchSpec, RootFS};
use chariot_util::{
    current_timestamp,
    fs::{dir_entries, force_rm, force_rm_contents, join_soft, make_path},
};
use git2::{FetchOptions, build::RepoBuilder};

use crate::{
    BuildServerState, REPOSITORIES_DIR, ROOTFS_DIR, STORE_DIR, WORKDIRS_DIR, api::events::BuildServerEvent, config::ProjectConfig,
    tracer::BuildServerTracer,
};

#[derive(Clone, Copy)]
pub enum JobTaskStatus {
    Pending,
    InProgress,
    Failed,
    Succeeded,
    Skipped,
}

impl ToString for JobTaskStatus {
    fn to_string(&self) -> String {
        String::from(match self {
            JobTaskStatus::Pending => "pending",
            JobTaskStatus::InProgress => "in_progress",
            JobTaskStatus::Failed => "failed",
            JobTaskStatus::Succeeded => "succeeded",
            JobTaskStatus::Skipped => "skipped",
        })
    }
}

// @todo: I'm sure there is a better way to do this
impl TryFrom<i64> for JobTaskStatus {
    type Error = i64;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(JobTaskStatus::Pending),
            1 => Ok(JobTaskStatus::InProgress),
            2 => Ok(JobTaskStatus::Failed),
            3 => Ok(JobTaskStatus::Succeeded),
            4 => Ok(JobTaskStatus::Skipped),
            _ => Err(value),
        }
    }
}
impl From<JobTaskStatus> for i64 {
    fn from(value: JobTaskStatus) -> Self {
        match value {
            JobTaskStatus::Pending => 0,
            JobTaskStatus::InProgress => 1,
            JobTaskStatus::Failed => 2,
            JobTaskStatus::Succeeded => 3,
            JobTaskStatus::Skipped => 4,
        }
    }
}

#[derive(Clone, Copy)]
pub enum JobTaskKind {
    Source,
    Package,
    Tool,
}

// @todo: I'm sure there is a better way to do this
impl TryFrom<i64> for JobTaskKind {
    type Error = i64;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(JobTaskKind::Source),
            1 => Ok(JobTaskKind::Package),
            2 => Ok(JobTaskKind::Tool),
            _ => Err(value),
        }
    }
}
impl From<JobTaskKind> for i64 {
    fn from(value: JobTaskKind) -> Self {
        match value {
            JobTaskKind::Source => 0,
            JobTaskKind::Package => 1,
            JobTaskKind::Tool => 2,
        }
    }
}

impl ToString for JobTaskKind {
    fn to_string(&self) -> String {
        String::from(match self {
            JobTaskKind::Source => "source",
            JobTaskKind::Package => "package",
            JobTaskKind::Tool => "tool",
        })
    }
}

pub struct JobTask {
    pub name: String,
    pub kind: JobTaskKind,
    pub status: JobTaskStatus,
}

pub enum JobStatus {
    Pending,
}

pub struct Job {
    pub id: i64,
    pub project: String,
    pub tasks: RwLock<HashMap<usize, JobTask>>,
}

pub fn run_project_build(state: &Arc<BuildServerState>, store: &Arc<Store>, project_name: &String, project_config: &ProjectConfig) -> Result<()> {
    let repos_dir = state.data_dir.join(REPOSITORIES_DIR);
    let repo_path = repos_dir.join(&project_name);

    make_path(&repo_path).context("Failed to make repository path")?;
    force_rm_contents(&repo_path, None).context("Failed to clean repository path")?;

    let mut fetch_options = FetchOptions::new();
    fetch_options.depth(1);

    let mut repo_builder = RepoBuilder::new();
    repo_builder.fetch_options(fetch_options);
    if let Some(branch) = &project_config.git_repository.branch {
        repo_builder.branch(branch);
    }

    repo_builder.clone(&project_config.git_repository.url, &repo_path)?;

    let base_config_path = join_soft(
        &repo_path,
        project_config
            .base_config_path
            .as_ref()
            .map(|str| str.as_str())
            .unwrap_or(DEFAULT_BASE_CONFIG_PATH),
    );

    let project_root = base_config_path.parent().unwrap_or(&repo_path);

    let base_config = read_base_config(&base_config_path).context("Failed to read base config")?;

    state
        .db
        .update_project_rootfs(project_name, &base_config.rootfs.hash)
        .context("Failed to update project rootfs hash")?;

    let rootfs_path = state.data_dir.join(ROOTFS_DIR).join(&base_config.rootfs.hash);

    let rootfs = match RootFS::get(&rootfs_path).context("Failed to get rootfs")? {
        Some(rootfs) => rootfs,
        None => RootFS::init(
            &rootfs_path,
            &ManifestFetchSpec {
                url: base_config.rootfs.url.unwrap_or(DEFAULT_MANIFESTS_URL.to_string()),
                version: base_config.rootfs.version,
                hash: base_config.rootfs.hash.clone(),
            },
            &mut stdout(),
        )
        .context("Failed to initialize rootfs")?,
    };
    let rootfs = Arc::new(rootfs);

    let mut binary_to_pkgset: HashMap<&str, Option<Arc<CachedPkgSet>>> = HashMap::new();
    for binary in ["bsdtar", "git", "patch", "sha256sum", "wget"] {
        let Some(pkg) = rootfs.lookup_package_of_binary(binary) else {
            bail!("This rootfs manifest is missing a required package mapping for the `{}` binary", binary);
        };

        binary_to_pkgset.insert(binary, CachedPkgSet::get(&rootfs, &None, &BTreeSet::from([pkg]), &mut stdout())?);
    }

    let workdir_parent = Arc::new(WorkDirectoryParent::get(state.data_dir.join(WORKDIRS_DIR)).context("Failed to get workdirs parent")?);

    // TODO: we are creating a build cache into a work directory because we do not
    // use a build cache here, it should only be optionally required by core
    let build_cache_workdir = WorkDirectory::create(&workdir_parent)?;
    let build_cache = Arc::new(BuildCache::get(build_cache_workdir.path())?);

    let core_context = CoreContext {
        parallelism: NonZero::new(1).unwrap(),
        rootfs,
        workdir_parent,
        ledger: state.ledger.clone(),
        store: store.clone(),
        build_cache,
        build_cache_enabled: HashSet::new(),
        bsdtar_pkgset: binary_to_pkgset.remove("bsdtar").unwrap(),
        git_pkgset: binary_to_pkgset.remove("git").unwrap(),
        patch_pkgset: binary_to_pkgset.remove("patch").unwrap(),
        sha256sum_pkgset: binary_to_pkgset.remove("sha256sum").unwrap(),
        wget_pkgset: binary_to_pkgset.remove("wget").unwrap(),
    };

    // let worker_count = available_parallelism().unwrap().div_ceil(NonZero::new(2).unwrap());
    let worker_count = NonZero::new(2).unwrap();

    for profile in &project_config.profiles {
        let job_id = state.db.create_job(
            project_name,
            &profile.target_arch,
            profile.options.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect(),
        )?;

        let job = Arc::new(Job {
            id: job_id,
            project: project_name.clone(),
            tasks: RwLock::new(HashMap::new()),
        });

        *state.current_job.write().unwrap() = Some(job.clone());
        let _ = state.event_channel.send(BuildServerEvent::JobStart(job_id));

        let target_prefix = base_config.target_prefix.clone().unwrap_or_else(|| String::from(DEFAULT_TARGET_PREFIX));

        let global_environment = Arc::new(GlobalEnvironment {
            global_environment_variables: base_config.global_environment_variables.clone(),
            global_native_packages: base_config.global_native_packages.clone(),
            rootfs_manifest_hash: base_config.rootfs.hash.clone(),
            target_prefix,
            target_arch: profile.target_arch.clone(),
        });

        let lua_config_path = join_soft(
            project_root,
            base_config.lua_root.clone().unwrap_or(PathBuf::from(DEFAULT_LUA_CONFIG_PATH)),
        );

        let local_sources_workdir = WorkDirectory::create(&core_context.workdir_parent)?;

        let config = eval_lua_config(
            lua_config_path,
            project_root,
            global_environment,
            profile.options.clone(),
            local_sources_workdir.path(),
            Vec::new(),
        )
        .context("Failed to evaluate lua config")?;

        let mut graph_builder = BuildGraphBuilder::new();
        for (name, platform) in iter::chain(
            project_config.build_packages.iter().zip(iter::repeat(PackagePlatform::Target)),
            project_config.build_tools.iter().zip(iter::repeat(PackagePlatform::Host)),
        ) {
            let package = config
                .packages
                .iter()
                .find(|package| &package.name == name && package.platform == platform);

            let package = match package {
                Some(package) => package,
                None => bail!("Config does not contain a {} package `{}`", platform.to_string(), name),
            };

            graph_builder.add_root_package(package);
        }

        let build_graph = graph_builder.finish();

        let tracer = BuildServerTracer::new(state.clone(), job.clone());
        let build_manager = BuildManager::new(&core_context, build_graph, Arc::new(tracer));
        let report = build_manager.execute(FailureMode::KeepGoing, worker_count);

        let _ = state.event_channel.send(BuildServerEvent::JobEnd);
        // @todo: we need to handle if a job fails *mid* way, right now we snowball the error to the caller and that probably just panics :^)
        state
            .db
            .finalize_job(job.id.clone(), &job.tasks.read().unwrap())
            .context("Failed to finalize job")?;
        *state.current_job.write().unwrap() = None;
    }

    Ok(())
}

pub fn run_build(state: &Arc<BuildServerState>) -> Result<()> {
    let store = Arc::new(Store::get(state.data_dir.join(STORE_DIR)).context("Failed to get store")?);

    for (project_name, project_config) in &state.config.projects {
        if project_config.disabled {
            continue;
        }

        run_project_build(state, &store, project_name, project_config)?;
    }

    state
        .db
        .filter_projects(state.config.projects.keys())
        .context("Failed to filter projects")?;

    let rootfs_hashes = state.db.get_all_rootfs_hashes()?;
    for entry in dir_entries(state.data_dir.join(ROOTFS_DIR))? {
        if rootfs_hashes.contains(&entry.file_name().to_string_lossy().to_string()) {
            continue;
        }

        force_rm(entry.path()).context("Failed to remove stale rootfs")?;
    }

    Ok(())
}
