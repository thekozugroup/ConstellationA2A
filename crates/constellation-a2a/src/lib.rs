//! Constellation A2A wire types — pure (de)serialization, no I/O.

pub mod card;
pub mod error;
pub mod rpc;
pub mod task;

pub use card::{AgentCapabilities, AgentCard, Skill};
pub use error::JsonRpcError;
pub use rpc::{JsonRpcRequest, JsonRpcResponse, JsonRpcVersion};
pub use task::{
    Message, Part, Role, TaskGetParams, TaskGetResult, TaskSendParams, TaskSendResult, TaskState,
    TaskStatus,
};

/// Canonical default A2A port. Used by both the CLI's bind defaults and the
/// Tailscale discoverer's probe URL construction.
pub const DEFAULT_PORT: u16 = 7777;
