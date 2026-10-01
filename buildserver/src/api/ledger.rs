use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};

use crate::{
    BuildServerState,
    api::{ApiError, ApiResult},
};

pub async fn lookup(State(state): State<Arc<BuildServerState>>, Path((category, input_hash)): Path<(String, String)>) -> ApiResult<Json<Value>> {
    let input_hash = u128::from_str_radix(&input_hash, 16).map_err(|err| ApiError::bad_request(err))?;
    let output_hash = state.ledger.lookup(category.as_ref(), input_hash)?;
    Ok(Json(json!({ "output_hash": output_hash.map(|hash| format!("{:x}", hash)) })))
}
