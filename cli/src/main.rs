use std::process::exit;

use colored::{Color, Colorize};
use log::{Level, LevelFilter, Log, error};
use nix::{
    sys::signal::{SigHandler, Signal, kill, signal},
    unistd::Pid,
};

use crate::cli::run_cli;

mod cli;
mod config;
mod util;

const LOGGER: ChariotLogger = ChariotLogger;

struct ChariotLogger;

impl Log for ChariotLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= Level::Error
    }

    fn log(&self, record: &log::Record) {
        let level_color = match record.level() {
            Level::Trace => Color::Black,
            Level::Debug => Color::Blue,
            Level::Info => Color::Green,
            Level::Warn => Color::Yellow,
            Level::Error => Color::Red,
        };

        eprintln!("{} | {}", record.level().as_str().color(level_color).bold(), record.args());
    }

    fn flush(&self) {}
}

extern "C" fn handle_sigint(_: nix::libc::c_int) {
    let _ = kill(Pid::from_raw(0), Signal::SIGKILL);
    exit(1);
}

fn main() {
    unsafe { signal(Signal::SIGINT, SigHandler::Handler(handle_sigint)) }.unwrap();

    log::set_logger(&LOGGER)
        .map(|()| log::set_max_level(LevelFilter::Info))
        .expect("Failed to initialize logger");

    if let Err(err) = run_cli() {
        error!("{}", err);
        if err.chain().len() > 1 {
            error!("Caused by:");
            for (i, sub_error) in err.chain().skip(1).enumerate() {
                for (j, line) in sub_error.to_string().lines().enumerate() {
                    if j == 0 {
                        error!("  {}: {}", i, line);
                    } else {
                        error!("     {}", line);
                    }
                }
            }
        }

        exit(1);
    }
}
