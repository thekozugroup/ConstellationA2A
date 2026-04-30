use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single content unit within a message (currently only plain text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Part {
    /// Plain UTF-8 text content.
    Text {
        /// The text body.
        text: String,
    },
}

/// A single message exchanged in a task, with a role and one or more parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Whether this message originated from a user or an agent.
    pub role: Role,
    /// Ordered list of content parts.
    pub parts: Vec<Part>,
}

/// Identifies the originator of a [`Message`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Message sent by a human or calling system.
    User,
    /// Message produced by an agent.
    Agent,
}

/// Lifecycle state of a task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    /// Task received; not yet started.
    Submitted,
    /// Task is actively being processed.
    Working,
    /// Agent needs more information before continuing.
    InputRequired,
    /// Task finished successfully.
    Completed,
    /// Task was cancelled.
    Canceled,
    /// Task failed.
    Failed,
    /// State not recognized.
    Unknown,
}

impl TaskState {
    /// Return the canonical wire string for this state.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Working => "working",
            Self::InputRequired => "input-required",
            Self::Completed => "completed",
            Self::Canceled => "canceled",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }

    /// Parse a wire string into a `TaskState`, defaulting to `Unknown`.
    pub fn parse(s: &str) -> Self {
        match s {
            "submitted" => Self::Submitted,
            "working" => Self::Working,
            "input-required" => Self::InputRequired,
            "completed" => Self::Completed,
            "canceled" => Self::Canceled,
            "failed" => Self::Failed,
            _ => Self::Unknown,
        }
    }
}

/// Snapshot of a task's current state at a point in time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStatus {
    /// Current lifecycle state.
    pub state: TaskState,
    /// When this status was recorded.
    pub timestamp: DateTime<Utc>,
}

/// Parameters for the `tasks/send` method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSendParams {
    /// Caller-chosen stable task identifier.
    pub id: String,
    /// Initial message to submit with the task.
    pub message: Message,
}

/// Parameters for the `tasks/get` method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGetParams {
    /// Task identifier to look up.
    pub id: String,
}

/// Result returned by both `tasks/send` and `tasks/get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGetResult {
    /// The task identifier.
    pub id: String,
    /// Latest known status.
    pub status: TaskStatus,
    /// Ordered message history for the task.
    #[serde(default)]
    pub history: Vec<Message>,
}
