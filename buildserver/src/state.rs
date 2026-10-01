use std::{
    collections::HashMap,
    fs::{exists, read_to_string, write},
    path::Path,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ProjectState {
    pub rootfs_hash: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub projects: HashMap<String, ProjectState>,
}

pub fn read_state(path: &Path) -> Result<State> {
    if !exists(path)? {
        return Ok(State::default());
    }

    let data = read_to_string(path)?;
    serde_json::from_str::<State>(&data).context("Failed to serialize state")
}

pub fn write_state(path: &Path, state: &State) -> Result<()> {
    let data = serde_json::to_string(&state).context("Failed to deserialize state")?;
    write(path, data)?;
    Ok(())
}
