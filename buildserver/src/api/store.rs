use std::{collections::HashMap, io::ErrorKind, sync::Arc};

use axum::{
    Json,
    extract::{Query, State},
};
use chariot_util::fs::dir_entries;
use serde_json::{Value, json};

use crate::{
    BuildServerState, STORE_DIR,
    api::{ApiError, ApiResult},
};

pub async fn index(State(state): State<Arc<BuildServerState>>, Query(query): Query<HashMap<String, String>>) -> ApiResult<Json<Value>> {
    let store_root = state.data_dir.join(STORE_DIR);

    let dir = match query.get("path") {
        Some(path) => {
            let relative = path.trim_start_matches('/');
            let dir = match store_root.join(relative).canonicalize() {
                Ok(dir) => dir,
                Err(err) if err.kind() == ErrorKind::NotFound => return Err(ApiError::not_found()),
                Err(err) => return Err(err.into()),
            };
            dir
        }
        None => store_root.clone(),
    };

    if !dir.starts_with(&store_root) || !dir.is_dir() {
        return Err(ApiError::not_found());
    }

    let mut entries = dir_entries(&dir)?
        .iter()
        .map(|entry| {
            let kind = entry.file_type().map(|kind| if kind.is_dir() { "directory" } else { "file" });
            json!({
                "name": entry.file_name().to_string_lossy(),
                "type": kind.unwrap_or("unknown"),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));

    Ok(Json(json!({
        "entries": entries,
    })))
}
