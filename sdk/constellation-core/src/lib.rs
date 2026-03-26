//! # constellation-core
//!
//! Core Rust SDK for Constellation A2A — agent-to-agent communication over Matrix.
//!
//! Provides [`ConstellationAgent`] for connecting to a Matrix homeserver, joining rooms,
//! sending messages with @-mentions, and dispatching structured task events between agents.

pub mod agent;
pub mod config;
pub mod error;
pub mod message;
pub mod room;
pub mod task;

// Re-export primary types at crate root for convenience.
pub use agent::ConstellationAgent;
pub use config::{AgentConfig, AgentConfigBuilder};
pub use error::{ConstellationError, Result};
pub use message::{
    ConstellationMetadata, MentionEvent, Message, MessageEvent, Priority, Task, TaskEvent,
    TaskResult, TaskStatus,
};
pub use room::RoomHandle;
pub use task::TaskManager;
