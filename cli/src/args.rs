use std::{num::NonZero, path::PathBuf, thread::available_parallelism};

use chariot_config::DEFAULT_BASE_CONFIG_PATH;
use chariot_core::{config::script::ScriptLanguage, executor::FailureMode};
use chariot_rootfs::DEFAULT_MANIFESTS_URL;
use clap::{Args, Parser, Subcommand, ValueEnum, value_parser};
use clap_complete::Shell;

const DEFAULT_CACHE_PATH: &str = ".chariot-cache";
const DEFAULT_ROOTFS_PATH: &str = ".chariot-rootfs";

const ARG_CACHE_HELP: &str = "path to chariot cache";
const ARG_CACHE_ENV: &str = "CHARIOT_CACHE_PATH";
const ARG_ROOTFS_HELP: &str = "path to chariot rootfs";
const ARG_ROOTFS_ENV: &str = "CHARIOT_ROOTFS_PATH";
const ARG_BASECONFIG_HELP: &str = "path to chariot base config";
const ARG_BASECONFIG_ENV: &str = "CHARIOT_BASE_CONFIG_PATH";

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum LocalConfigFormat {
    Toml,
    Json,
}

#[derive(Parser)]
#[command(version, next_line_help = true)]
pub struct ChariotOptions {
    #[arg(long, help = "path to local config", default_value = ".chariot.toml")]
    pub local_config: String,

    #[arg(long, help = "format of the local config", default_value = "toml")]
    pub local_config_format: LocalConfigFormat,

    #[command(subcommand)]
    pub command: MainCommand,
}

#[derive(Subcommand)]
pub enum MainCommand {
    #[command(about = "install package")]
    Install(InstallOptions),

    #[command(about = "execute a command inside provided environment")]
    Exec(ExecOptions),

    #[command(about = "check whether a package or tool is already present in the store")]
    Lookup(LookupOptions),

    #[command(about = "build package(s) without installing them")]
    Build(BuildOptions),

    #[command(about = "lsp command")]
    Lsp(LspOptions),

    #[command(about = "cache support commands")]
    Cache(CacheOptions),

    #[command(about = "rootfs support commands")]
    Rootfs(RootFSOptions),

    #[command(about = "miscellaneous support tooling")]
    Support {
        #[command(subcommand)]
        command: SupportCommand,
    },
}

#[derive(Args)]
pub struct CacheOptions {
    #[arg(long, env = ARG_CACHE_ENV, help = ARG_CACHE_HELP,  default_value = DEFAULT_CACHE_PATH)]
    pub cache: PathBuf,

    #[command(subcommand)]
    pub command: CacheCommand,
}

#[derive(Subcommand)]
pub enum CacheCommand {
    #[command(about = "lists all entries in the ledger")]
    ListLedger,

    #[command(about = "deletes all store and ledger entries")]
    Purge,

    #[command(about = "garbage collect")]
    Gc {
        #[arg(long, env = ARG_BASECONFIG_ENV, help = ARG_BASECONFIG_HELP,  default_value = DEFAULT_BASE_CONFIG_PATH)]
        base_config: PathBuf,
    },
}

#[derive(Args)]
pub struct RootFSOptions {
    #[arg(long, env = ARG_ROOTFS_ENV, help = ARG_ROOTFS_HELP, default_value = DEFAULT_ROOTFS_PATH)]
    pub rootfs: PathBuf,

    #[command(subcommand)]
    pub command: RootFSCommand,
}

#[derive(Subcommand)]
pub enum RootFSCommand {
    Init {
        #[arg(long, default_value = DEFAULT_MANIFESTS_URL)]
        url: String,
        version: String,
        hash: String,
    },
    Status,
    #[command(subcommand)]
    Pkgset(PkgsetCommand),
    Purge,
}

#[derive(Subcommand)]
pub enum PkgsetCommand {
    List,
    Cache { packages: Vec<String> },
}

#[derive(Subcommand)]
pub enum SupportCommand {
    #[command(about = "generate lua lsp configuration")]
    SetupLSP {
        #[arg(long, help = "directory to place lua bindings in", default_value_os_t = default_lsp_support_path())]
        support_dir: PathBuf,
    },

    #[command(about = "generate shell completions for chariot")]
    Completions {
        #[arg(help = "shell to generate completions for", value_parser = value_parser!(Shell))]
        shell: Shell,
    },
}

#[derive(Args)]
pub struct ConfigOptions {
    #[arg(long, env = ARG_CACHE_ENV, help = ARG_CACHE_HELP, default_value = DEFAULT_CACHE_PATH)]
    pub cache: PathBuf,

    #[arg(long, env = ARG_BASECONFIG_ENV, help = ARG_BASECONFIG_HELP, default_value = DEFAULT_BASE_CONFIG_PATH)]
    pub base_config: PathBuf,

    #[arg(long, env = "CHARIOT_ARCH", help = "target architecture")]
    pub arch: String,

    #[arg(long, short, env = "CHARIOT_OPTIONS", help = "user defined option", value_parser = parse_kv, value_delimiter = ',')]
    pub option: Vec<(String, String)>,

    #[arg(long, help = "allow creation of new profiles without user input")]
    pub allow_new_profiles: bool,
}

#[derive(Args)]
pub struct CommonBuildOptions {
    #[command(flatten)]
    pub config_opts: ConfigOptions,

    #[arg(long, env = ARG_ROOTFS_ENV, help = ARG_ROOTFS_HELP, default_value = DEFAULT_ROOTFS_PATH)]
    pub rootfs: PathBuf,

    #[arg(
        long,
        short = 'j',
        help = "total job budget shared across all tasks via a jobserver, passed to scripts as PARALLELISM/MAKEFLAGS",
        default_value_t = available_parallelism().unwrap()
    )]
    pub parallelism: NonZero<usize>,
}

#[derive(Args)]
pub struct ExecutionOptions {
    #[arg(long, help = "keep building unrelated packages after a failure instead of stopping immediately")]
    pub keep_going: bool,
}

impl ExecutionOptions {
    pub fn failure_mode(&self) -> FailureMode {
        if self.keep_going {
            FailureMode::KeepGoing
        } else {
            FailureMode::FailFast
        }
    }
}

#[derive(Args)]
pub struct ExecOptions {
    #[command(flatten)]
    pub common_build_opts: CommonBuildOptions,

    #[command(flatten)]
    pub execution_opts: ExecutionOptions,

    #[arg(long, help = "make execution environment reflect the build environment of a package")]
    pub build_env: Option<String>,

    #[arg(short, long, help = "mount package build directory into execution environment", value_name = "DEST_PATH=PACKAGE_NAME", value_parser = parse_kv)]
    pub build_dir: Vec<(String, String)>,

    #[arg(
        short = 'p',
        long,
        help = "native packages to install into the execution environment",
        value_delimiter = ','
    )]
    pub native_pkg: Vec<String>,

    #[arg(long, help = "host packages to install into the execution environment", value_delimiter = ',')]
    pub tool: Vec<String>,

    #[arg(long, help = "target packages to install into the sysroot", value_delimiter = ',')]
    pub pkg: Vec<String>,

    #[arg(long, help = "current working directory", default_value = "/")]
    pub cwd: String,

    #[arg(short, long, help = "environment variable(s) to pass into the execution environment", value_parser = parse_kv, value_delimiter = ',')]
    pub env_var: Vec<(String, String)>,

    #[arg(short, long, help = "bind mount(s) into the execution environment", value_parser = parse_mount)]
    pub mount: Vec<(String, String, bool, bool)>,

    #[arg(long, help = "script language", default_value = "bash", value_parser = parse_language)]
    pub language: ScriptLanguage,

    #[arg(long, help = "forward stdin into execution environment")]
    pub stdin: bool,

    #[arg(long, help = "forward stdout from execution environment")]
    pub no_stdout: bool,

    #[arg(long, help = "forward stderr from execution environment")]
    pub no_stderr: bool,

    #[arg(help = "script to execute")]
    pub command: String,
}

#[derive(Args)]
pub struct InstallOptions {
    #[command(flatten)]
    pub common_build_opts: CommonBuildOptions,

    #[command(flatten)]
    pub execution_opts: ExecutionOptions,

    #[arg(long, help = "install a host package (tool)")]
    pub tool: bool,

    #[arg(long, help = "force installation, even if the package is already installed")]
    pub force: bool,

    #[arg(required = true, help = "packages to build and install")]
    pub packages: Vec<String>,

    #[arg(required = true, help = "package install destination")]
    pub dest: String,
}

#[derive(Args)]
pub struct BuildOptions {
    #[command(flatten)]
    pub common_build_opts: CommonBuildOptions,

    #[command(flatten)]
    pub execution_opts: ExecutionOptions,

    #[arg(long, help = "build a host package (tool) instead of a target package")]
    pub tool: bool,

    #[arg(required = true, help = "packages to build")]
    pub packages: Vec<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum SupportedLsp {
    Clangd,
}

#[derive(Args)]
pub struct LspOptions {
    #[command(flatten)]
    pub common_build_opts: CommonBuildOptions,

    #[command(flatten)]
    pub execution_opts: ExecutionOptions,

    #[arg(long, short = 'm', help = "source mappings", value_parser = parse_kv)]
    pub source_mappings: Vec<(String, String)>,

    #[arg(help = "the lsp to use")]
    pub lsp: SupportedLsp,

    #[arg(help = "the package to run the lsp for")]
    pub package: String,
}

#[derive(Args)]
pub struct LookupOptions {
    #[command(flatten)]
    pub config_opts: ConfigOptions,

    #[arg(long, help = "look up a host package (tool) instead of a target package")]
    pub tool: bool,

    #[arg(help = "package or tool name to look up")]
    pub name: String,
}

fn default_lsp_support_path() -> PathBuf {
    dirs::data_dir()
        .map(|dir| dir.join("chariot").join("lsp-support"))
        .unwrap_or(PathBuf::from(".chariot-lsp-support"))
}

fn parse_kv(str: &str) -> Result<(String, String), String> {
    let pos = str.find('=').ok_or_else(|| format!("invalid KEY=VALUE: no `=` found in `{}`", str))?;
    Ok((str[..pos].to_string(), str[pos + 1..].to_string()))
}

fn parse_language(str: &str) -> Result<ScriptLanguage, String> {
    match str {
        "bash" | "sh" => Ok(ScriptLanguage::Bash),
        "python" | "py" => Ok(ScriptLanguage::Python),
        str => Err(format!("unknown script language `{}`", str)),
    }
}

fn parse_mount(str: &str) -> Result<(String, String, bool, bool), String> {
    let (mount, opts) = match str.split_once(":") {
        Some((mount, opts)) => (mount, opts.split(":").collect::<Vec<_>>()),
        None => (str, Vec::new()),
    };

    let mut is_read_only = false;
    let mut is_file = false;
    for opt in opts {
        match opt {
            "ro" => is_read_only = true,
            "file" => is_file = true,
            _ => continue,
        }
    }

    match mount.split_once("=") {
        None => Err(format!("`{}` is not a valid mount", str)),
        Some((from, to)) => Ok((from.to_string(), to.to_string(), is_read_only, is_file)),
    }
}
