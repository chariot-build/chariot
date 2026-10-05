use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::{
    BuildServerState,
    api::{ApiError, ApiResult},
};

pub async fn get_current(State(state): State<Arc<BuildServerState>>) -> Json<Value> {
    match state.current_job.read().unwrap().as_ref() {
        None => Json(json!({ "id": Value::Null })),
        Some(job) => Json(json!({ "id": job.id })),
    }
}

pub async fn get_project(State(state): State<Arc<BuildServerState>>, Path(project): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "jobs": state.db.get_project_jobs(&project)? })))
}

pub async fn get(State(state): State<Arc<BuildServerState>>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    if let Some(job) = state.current_job.read().unwrap().as_ref()
        && job.id == id
    {
        let tasks = job
            .tasks
            .read()
            .unwrap()
            .iter()
            .map(|(id, task)| {
                json!({
                    "id": id,
                    "type": &task.kind.to_string(),
                    "name": task.name,
                    "status": task.status.to_string(),
                    "input_hash": format!("{:x}", task.input_hash)
                })
            })
            .collect::<Vec<_>>();

        return Ok(Json(json!({
            "active": true,
            "project": &job.project,
            "tasks": tasks
        })));
    }

    let job_project = match state.db.get_job_project(id)? {
        Some(job) => job,
        None => return Err(ApiError::not_found()),
    };

    let tasks = state.db.get_job_tasks(id)?;

    // @todo: maybe merge this with the above logic? somehow?
    let tasks = tasks
        .iter()
        .map(|task| {
            json!({
                "id": id,
                "type": &task.kind.to_string(),
                "name": task.name,
                "status": task.status.to_string(),
                "input_hash": format!("{:x}", task.input_hash)
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "active": false,
        "project": job_project,
        "tasks": tasks
    })))
}
