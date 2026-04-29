use crate::error::JsonRpcError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: JsonRpcVersion,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonRpcVersion(String);

impl Default for JsonRpcVersion {
    fn default() -> Self {
        Self("2.0".into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: JsonRpcVersion,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl<T> JsonRpcResponse<T> {
    pub fn ok(id: serde_json::Value, result: T) -> Self {
        Self {
            jsonrpc: JsonRpcVersion::default(),
            id,
            result: Some(result),
            error: None,
        }
    }
    pub fn err(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JsonRpcVersion::default(),
            id,
            result: None,
            error: Some(error),
        }
    }
}
