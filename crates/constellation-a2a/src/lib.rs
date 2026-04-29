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
