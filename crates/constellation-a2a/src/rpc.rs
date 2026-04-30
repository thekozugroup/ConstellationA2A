use crate::error::JsonRpcError;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

const JSONRPC_VERSION: &str = "2.0";

/// The literal JSON-RPC version string `"2.0"`. Validates on deserialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonRpcVersion;

impl Serialize for JsonRpcVersion {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(JSONRPC_VERSION)
    }
}

impl<'de> Deserialize<'de> for JsonRpcVersion {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s == JSONRPC_VERSION {
            Ok(Self)
        } else {
            Err(de::Error::custom(format!(
                "expected jsonrpc \"2.0\", got \"{s}\""
            )))
        }
    }
}

/// A JSON-RPC 2.0 request envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Must equal `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// Request identifier echoed back in the response.
    pub id: serde_json::Value,
    /// Method name, e.g. `"tasks/send"`.
    pub method: String,
    /// Method parameters; defaults to `null`.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// A JSON-RPC 2.0 response envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    /// Must equal `"2.0"`.
    pub jsonrpc: JsonRpcVersion,
    /// Echoed from the corresponding request.
    pub id: serde_json::Value,
    /// Successful result payload; absent when `error` is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    /// Error payload; absent when `result` is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl<T> JsonRpcResponse<T> {
    /// Build a successful response with the given result.
    pub fn ok(id: serde_json::Value, result: T) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Build an error response with the given JSON-RPC error.
    pub fn err(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JsonRpcVersion,
            id,
            result: None,
            error: Some(error),
        }
    }
}
