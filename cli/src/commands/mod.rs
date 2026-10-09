use anyhow::{Context, Result};
use clap::Parser;

use crate::{
    args::{ChariotOptions, MainCommand},
    cli_config::parse_cli_config,
    commands,
};

pub mod build;
pub mod cache;
pub mod exec;
pub mod install;
pub mod lookup;
pub mod lsp;
pub mod rootfs;
pub mod support;

pub fn run_cli() -> Result<()> {
    let opts = ChariotOptions::parse();

    let local_config = parse_cli_config(&opts.local_config, opts.local_config_format).context("Failed to parse local config")?;

    match opts.command {
        MainCommand::Install(install_opts) => commands::install::run(install_opts, &local_config),
        MainCommand::Lookup(lookup_opts) => commands::lookup::run(lookup_opts, &local_config),
        MainCommand::Build(build_opts) => commands::build::run(build_opts, &local_config),
        MainCommand::Lsp(lsp_options) => commands::lsp::run(lsp_options, &local_config),
        MainCommand::Exec(exec_options) => commands::exec::run(exec_options, &local_config),
        MainCommand::Cache(cache_options) => commands::cache::run(cache_options, &local_config),
        MainCommand::Rootfs(rootfs_options) => commands::rootfs::run(rootfs_options),
        MainCommand::Support { command } => commands::support::run(command),
    }
}
