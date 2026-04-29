//! A2A JSON-RPC client used to send tasks to remote peers.

use anyhow::{anyhow, Result};
use constellation_a2a::{
    JsonRpcRequest, JsonRpcResponse, JsonRpcVersion, Message, TaskGetParams, TaskGetResult,
    TaskSendParams,
};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

#[derive(Clone)]
pub struct A2aClient {
    http: Client,
}

impl Default for A2aClient {
    fn default() -> Self {
        Self::new()
    }
}

impl A2aClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client builds");
        Self { http }
    }

    pub async fn send_task(
        &self,
        peer_url: &str,
        task_id: &str,
        message: &Message,
    ) -> Result<TaskGetResult> {
        let req = JsonRpcRequest {
            jsonrpc: JsonRpcVersion::default(),
            id: json!(uuid::Uuid::new_v4().to_string()),
            method: "tasks/send".into(),
            params: serde_json::to_value(TaskSendParams {
                id: task_id.into(),
                message: message.clone(),
            })?,
        };
        let resp: JsonRpcResponse<TaskGetResult> = self
            .http
            .post(peer_url)
            .json(&req)
            .send()
            .await?
            .json()
            .await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("peer returned JSON-RPC error: {}", err.message));
        }
        resp.result
            .ok_or_else(|| anyhow!("peer returned no result"))
    }

    pub async fn get_task(&self, peer_url: &str, task_id: &str) -> Result<TaskGetResult> {
        let req = JsonRpcRequest {
            jsonrpc: JsonRpcVersion::default(),
            id: json!(uuid::Uuid::new_v4().to_string()),
            method: "tasks/get".into(),
            params: serde_json::to_value(TaskGetParams { id: task_id.into() })?,
        };
        let resp: JsonRpcResponse<TaskGetResult> = self
            .http
            .post(peer_url)
            .json(&req)
            .send()
            .await?
            .json()
            .await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("peer returned JSON-RPC error: {}", err.message));
        }
        resp.result
            .ok_or_else(|| anyhow!("peer returned no result"))
    }
}
