use serde::{Deserialize, Serialize};

/// A JSON-RPC 2.0 error object carried in an error response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Numeric error code following the JSON-RPC spec conventions.
    pub code: i32,
    /// Human-readable error description.
    pub message: String,
    /// Optional structured error data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Create a parse error (`-32700`).
    pub fn parse_error() -> Self {
        Self {
            code: -32700,
            message: "Parse error".into(),
            data: None,
        }
    }

    /// Create an invalid-request error (`-32600`).
    pub fn invalid_request() -> Self {
        Self {
            code: -32600,
            message: "Invalid Request".into(),
            data: None,
        }
    }

    /// Create a method-not-found error (`-32601`) for the given method name.
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }

    /// Create an invalid-params error (`-32602`) with a custom message.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    /// Create an internal-error (`-32603`) with a custom message.
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
            data: None,
        }
    }

    /// Create a task-not-found error (`-32001`) for the given task ID.
    pub fn task_not_found(id: &str) -> Self {
        Self {
            code: -32001,
            message: format!("Task not found: {id}"),
            data: None,
        }
    }
}
