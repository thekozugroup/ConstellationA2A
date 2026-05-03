//! A2A JSON-RPC client used to send tasks to remote peers.

use anyhow::{anyhow, Result};
use constellation_a2a::{
    JsonRpcRequest, JsonRpcResponse, JsonRpcVersion, Message, TaskGetParams, TaskGetResult,
    TaskSendParams, SOURCE_URL_HEADER,
};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

/// HTTP client for sending A2A JSON-RPC requests to remote peers.
#[derive(Clone)]
pub struct A2aClient {
    http: Client,
    source_url: Option<String>,
}

impl Default for A2aClient {
    fn default() -> Self {
        Self::new()
    }
}

impl A2aClient {
    /// Create a new client with no source URL set.
    pub fn new() -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client builds");
        Self {
            http,
            source_url: None,
        }
    }

    /// Set the local agent URL to advertise on every outbound request.
    pub fn with_source_url(mut self, url: impl Into<String>) -> Self {
        self.source_url = Some(url.into());
        self
    }

    /// Attach the `X-A2A-Source-Url` header if a source URL is configured.
    fn install_source(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(url) = &self.source_url {
            builder = builder.header(SOURCE_URL_HEADER, url);
        }
        builder
    }

    /// Send a new task to `peer_url` and return the initial status.
    pub async fn send_task(
        &self,
        peer_url: &str,
        task_id: &str,
        message: &Message,
    ) -> Result<TaskGetResult> {
        let req = JsonRpcRequest {
            jsonrpc: JsonRpcVersion,
            id: json!(uuid::Uuid::new_v4().to_string()),
            method: "tasks/send".into(),
            params: serde_json::to_value(TaskSendParams {
                id: task_id.into(),
                message: message.clone(),
            })?,
        };
        let resp: JsonRpcResponse<TaskGetResult> = self
            .install_source(self.http.post(peer_url))
            .json(&req)
            .send()
            .await?
            .json()
            .await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("peer JSON-RPC error {}: {}", err.code, err.message));
        }
        resp.result
            .ok_or_else(|| anyhow!("peer returned no result"))
    }

    /// Poll the status of an existing task on `peer_url`.
    pub async fn get_task(&self, peer_url: &str, task_id: &str) -> Result<TaskGetResult> {
        let req = JsonRpcRequest {
            jsonrpc: JsonRpcVersion,
            id: json!(uuid::Uuid::new_v4().to_string()),
            method: "tasks/get".into(),
            params: serde_json::to_value(TaskGetParams { id: task_id.into() })?,
        };
        let resp: JsonRpcResponse<TaskGetResult> = self
            .install_source(self.http.post(peer_url))
            .json(&req)
            .send()
            .await?
            .json()
            .await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("peer JSON-RPC error {}: {}", err.code, err.message));
        }
        resp.result
            .ok_or_else(|| anyhow!("peer returned no result"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let client = A2aClient::new();
        assert_eq!(client.source_url, None);
    }

    #[test]
    fn test_default() {
        let client = A2aClient::default();
        assert_eq!(client.source_url, None);
    }

    #[test]
    fn test_with_source_url() {
        let client = A2aClient::new().with_source_url("http://example.com");
        assert_eq!(client.source_url, Some("http://example.com".to_string()));
    }
}
