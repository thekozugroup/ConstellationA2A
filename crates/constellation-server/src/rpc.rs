//! JSON-RPC dispatch handler for the A2A server.

use axum::{extract::State, http::HeaderMap, Json};
use chrono::Utc;
use constellation_a2a::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, Message, TaskGetParams, TaskGetResult,
    TaskSendParams, TaskState, TaskStatus, SOURCE_URL_HEADER,
};
use constellation_store::{tasks_in, Store};
use std::sync::Arc;

use crate::state::AppState;

/// Main JSON-RPC dispatch entry point; reads `X-A2A-Source-Url` from headers.
pub async fn dispatch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> Json<serde_json::Value> {
    let id = req.id.clone();
    let from_peer = headers
        .get(SOURCE_URL_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let outcome = match req.method.as_str() {
        "tasks/send" => handle_send(&state.store, &from_peer, req).await,
        "tasks/get" => handle_get(&state.store, req).await,
        // -32004: task subsystem is implemented, but cancel is not yet wired
        // (out-of-scope for v1 per spec). Distinct from -32601 method-not-found.
        "tasks/cancel" => Err(JsonRpcError::not_implemented("tasks/cancel")),
        other => Err(JsonRpcError::method_not_found(other)),
    };
    let response = match outcome {
        Ok(value) => JsonRpcResponse::<serde_json::Value>::ok(id, value),
        Err(e) => JsonRpcResponse::<serde_json::Value>::err(id, e),
    };
    Json(serde_json::to_value(response).unwrap_or_else(|_| {
        serde_json::json!({
            "jsonrpc":"2.0","id":null,
            "error":{"code":-32603,"message":"failed to encode response"}
        })
    }))
}

/// Handle a `tasks/send` request, recording `from_peer` into the store.
async fn handle_send(
    store: &Arc<Store>,
    from_peer: &str,
    req: JsonRpcRequest,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: TaskSendParams = serde_json::from_value(req.params)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    let store_for_blocking = store.clone();
    let from_peer_owned = from_peer.to_string();
    let id_for_blocking = params.id.clone();
    let msg_for_blocking = params.message.clone();
    tokio::task::spawn_blocking(move || {
        tasks_in::insert(
            &store_for_blocking,
            &id_for_blocking,
            &from_peer_owned,
            &msg_for_blocking,
        )
    })
    .await
    .map_err(|e| JsonRpcError::internal_error(format!("join error: {e}")))?
    .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;
    let result = TaskGetResult {
        id: params.id,
        status: TaskStatus {
            state: TaskState::Submitted,
            timestamp: Utc::now(),
        },
        history: vec![params.message],
    };
    serde_json::to_value(result).map_err(|e| JsonRpcError::internal_error(e.to_string()))
}

/// Handle a `tasks/get` request, returning the stored task or a not-found error.
async fn handle_get(
    store: &Arc<Store>,
    req: JsonRpcRequest,
) -> Result<serde_json::Value, JsonRpcError> {
    let params: TaskGetParams = serde_json::from_value(req.params)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    let store_for_blocking = store.clone();
    let id_for_blocking = params.id.clone();
    let task =
        tokio::task::spawn_blocking(move || tasks_in::get(&store_for_blocking, &id_for_blocking))
            .await
            .map_err(|e| JsonRpcError::internal_error(format!("join error: {e}")))?
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
            timestamp: task.updated_at,
        },
        history,
    };
    serde_json::to_value(result).map_err(|e| JsonRpcError::internal_error(e.to_string()))
}
