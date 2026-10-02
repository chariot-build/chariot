use std::{
    fs::read_to_string,
    path::PathBuf,
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use crate::{
    api::events::BuildServerEvent,
    config::BuildServerConfig,
    db::Database,
    job::{Job, run_build},
};
use anyhow::{Context, Result};
use axum::{Router, routing::get};
use chariot_core::ledger::Ledger;
use chariot_util::{current_timestamp, fs::make_path, lock::DirLock};
use std::fs::{rename, write};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

mod api;
mod config;
mod db;
mod job;
mod tracer;

const DATA_DIR: &str = "./data";
const REPOSITORIES_DIR: &str = "repositories";
const ROOTFS_DIR: &str = "rootfs";
const WORKDIRS_DIR: &str = "work";
const STORE_DIR: &str = "store";
const LEDGER_FILENAME: &str = "ledger.db";
const DATABASE_FILENAME: &str = "buildserver.db";
const LASTRUN_FILENAME: &str = "last_run.txt";
const LASTRUN_TMP_FILENAME: &str = "last_run.txt.tmp";

struct BuildServerState {
    data_dir: PathBuf,
    config: BuildServerConfig,
    ledger: Arc<Ledger>,
    db: Arc<Database>,
    current_job: RwLock<Option<Arc<Job>>>,
    event_channel: broadcast::Sender<BuildServerEvent>,
}

fn interval_handler(state: Arc<BuildServerState>) -> ! {
    let last_run_path = state.data_dir.join(LASTRUN_FILENAME);
    let mut last_run = read_to_string(&last_run_path).ok().map(|data| data.trim().parse::<u64>().ok()).flatten();

    loop {
        let now = current_timestamp();
        let due = match last_run {
            None => true,
            Some(last) => now >= last + state.config.interval,
        };

        if !due {
            continue;
        }

        println!("Running build");

        run_build(&state).expect("Build failed");

        let tmp_path = state.data_dir.join(LASTRUN_TMP_FILENAME);
        write(&tmp_path, now.to_string()).expect("Failed to write last run tmp");
        rename(&tmp_path, &last_run_path).expect("Failed to rename last run");

        last_run = Some(now);

        sleep(Duration::from_secs(10));
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

    let db = Database::get(data_dir.join(DATABASE_FILENAME)).context("Failed to open database")?;

    let (tx, _) = broadcast::channel::<BuildServerEvent>(100);
    let state = Arc::new(BuildServerState {
        data_dir,
        config,
        db: Arc::new(db),
        ledger: Arc::new(ledger),
        current_job: RwLock::new(None),
        event_channel: tx,
    });

    let app = Router::new()
        .route("/events", get(api::events::get_events))
        .route("/projects", get(api::meta::get_projects))
        .route("/project/{project}/jobs", get(api::jobs::get_project))
        .route("/job/active", get(api::jobs::get_current))
        .route("/job/{id}/details", get(api::jobs::get))
        .route("/ledger/lookup/{category}/{hash}", get(api::ledger::lookup))
        .layer(CorsLayer::permissive())
        .with_state(state.clone())
        .fallback_service(ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Listening on 0.0.0.0:3000 <3");

    std::thread::spawn(|| interval_handler(state));

    axum::serve(listener, app).await?;

    Ok(())
}
