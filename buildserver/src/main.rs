use std::{fs::read_to_string, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use axum::{Router, routing::get};
use chariot_core::ledger::Ledger;
use chariot_util::{current_timestamp, fs::make_path, lock::DirLock};
use tokio::time::{MissedTickBehavior, interval};
use tower_http::services::ServeDir;

use crate::{config::BuildServerConfig, job::run_build};

mod api;
mod config;
mod job;
mod state;
mod tracer;

const DATA_DIR: &str = "./data";
const REPOSITORIES_DIR: &str = "repositories";
const ROOTFS_DIR: &str = "rootfs";
const WORKDIRS_DIR: &str = "work";
const STORE_DIR: &str = "store";
const LEDGER_FILENAME: &str = "ledger.db";
const STATE_FILENAME: &str = "state.json";
const LASTRUN_FILENAME: &str = "last_run.txt";
const LASTRUN_TMP_FILENAME: &str = "last_run.txt.tmp";

struct BuildServerState {
    data_dir: PathBuf,
    config: BuildServerConfig,
    ledger: Arc<Ledger>,
}

async fn interval_handler(state: Arc<BuildServerState>) -> ! {
    let last_run_path = state.data_dir.join(LASTRUN_FILENAME);
    let mut last_run = tokio::fs::read_to_string(&last_run_path)
        .await
        .ok()
        .map(|data| data.trim().parse::<u64>().ok())
        .flatten();

    let mut tick = interval(Duration::from_secs(60));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tick.tick().await;

        let now = current_timestamp();
        let due = match last_run {
            None => true,
            Some(t) => now >= t + state.config.interval,
        };

        if !due {
            continue;
        }

        run_build(&state).expect("Build failed");

        let tmp_path = state.data_dir.join(LASTRUN_TMP_FILENAME);
        tokio::fs::write(&tmp_path, now.to_string()).await.expect("Failed to write last run tmp");
        tokio::fs::rename(&tmp_path, &last_run_path).await.expect("Failed to rename last run");

        last_run = Some(now);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = read_to_string("config.toml")?;
    let config = toml::from_str::<BuildServerConfig>(&config)?;

    make_path(DATA_DIR).context("Failed to make data directory path")?;

    let data_dir = PathBuf::from(DATA_DIR)
        .canonicalize()
        .context("Failed to canonicalize data directory path")?;

    let _data_dir_lock = DirLock::exclusive_noblock(&data_dir).context("Failed to lock data directory")?;

    let ledger = Ledger::get(data_dir.join(LEDGER_FILENAME)).context("Failed to get ledger")?;

    let state = Arc::new(BuildServerState {
        data_dir,
        config,
        ledger: Arc::new(ledger),
    });

    let app = Router::new()
        .route("/meta/projects", get(api::meta::get_projects))
        .route("/ledger/lookup/{category}/{hash}", get(api::ledger::lookup))
        .with_state(state.clone())
        .fallback_service(ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    let server = tokio::spawn(async move { axum::serve(listener, app).await });

    std::thread::spawn(|| interval_handler(state));

    server.await??;

    Ok(())
}
