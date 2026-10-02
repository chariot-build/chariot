use std::sync::Arc;

use axum::{Json, extract::State};
use serde_json::{Value, json};

use crate::BuildServerState;

pub async fn get_projects(State(state): State<Arc<BuildServerState>>) -> Json<Value> {
    let projects = state
        .config
        .projects
        .iter()
        .map(|(name, project_config)| json!({ "name": name, "repository": &project_config.git_repository.url }))
        .collect::<Vec<_>>();

    Json(json!({ "projects": projects }))
}
