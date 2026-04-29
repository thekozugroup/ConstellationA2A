use axum::{extract::State, Json};
use chrono::Utc;
use constellation_a2a::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, Message, TaskGetParams, TaskGetResult,
    TaskSendParams, TaskState, TaskStatus,
};
use constellation_store::{tasks_in, Store};
use std::sync::Arc;

use crate::state::AppState;

pub async fn dispatch(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<serde_json::Value> {
    let id = req.id.clone();
    let body = match req.method.as_str() {
        "tasks/send" => handle_send(&state.store, req).await,
        "tasks/get" => handle_get(&state.store, req).await,
        "tasks/cancel" => Err(JsonRpcError::method_not_found("tasks/cancel")),
        other => Err(JsonRpcError::method_not_found(other)),
    };
    let response = match body {
        Ok(value) => JsonRpcResponse::<serde_json::Value>::ok(id, value),
        Err(e) => JsonRpcResponse::<serde_json::Value>::err(id, e),
    };
    Json(serde_json::to_value(response).unwrap())
}

async fn handle_send(
    store: &Arc<Store>,
    req: JsonRpcRequest,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: TaskSendParams = serde_json::from_value(req.params)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    tasks_in::insert(store, &params.id, "unknown", &params.message)
        .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;
    let result = TaskGetResult {
        id: params.id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            timestamp: Utc::now(),
        },
        history: vec![params.message],
    };
    Ok(serde_json::to_value(result).unwrap())
}

async fn handle_get(
    store: &Arc<Store>,
    req: JsonRpcRequest,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: TaskGetParams = serde_json::from_value(req.params)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    let task = tasks_in::get(store, &params.id)
        .map_err(|e| JsonRpcError::internal_error(e.to_string()))?
        .ok_or_else(|| JsonRpcError::task_not_found(&params.id))?;
    let mut history: Vec<Message> = vec![task.request];
    if let Some(resp) = task.response {
        history.push(resp);
    }
    let result = TaskGetResult {
        id: task.task_id,
        status: TaskStatus {
            state: task.state,
            timestamp: Utc::now(),
        },
        history,
    };
    Ok(serde_json::to_value(result).unwrap())
}
