use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Priority level for tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// The `ai.constellation.metadata` block embedded in Matrix message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationMetadata {
    pub task_id: String,
    pub task_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub priority: Priority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

/// An outgoing message to be sent to a room.
#[derive(Debug, Clone)]
pub struct Message {
    pub body: String,
    pub metadata: Option<ConstellationMetadata>,
}

impl Message {
    pub fn text(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            metadata: None,
        }
    }

    pub fn with_metadata(mut self, metadata: ConstellationMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Event received when this agent is @-mentioned.
#[derive(Debug, Clone)]
pub struct MentionEvent {
    /// The Matrix user ID of the sender (e.g. `@agent-a:constellation.local`).
    pub sender: String,
    /// The room ID where the mention occurred.
    pub room_id: String,
    /// The plain-text body of the message.
    pub body: String,
    /// Parsed constellation metadata, if present.
    pub metadata: Option<ConstellationMetadata>,
    /// List of agent user IDs mentioned in this message.
    pub mentioned_agents: Vec<String>,
}

/// Event received for any message in a joined room.
#[derive(Debug, Clone)]
pub struct MessageEvent {
    pub sender: String,
    pub room_id: String,
    pub body: String,
    pub raw_event: serde_json::Value,
}

/// Event received when a structured task message arrives.
#[derive(Debug, Clone)]
pub struct TaskEvent {
    pub sender: String,
    pub room_id: String,
    pub task_id: String,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub priority: Priority,
}

/// A task to be created and sent to a room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(default = "generate_task_id")]
    pub id: String,
    pub task_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub priority: Priority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

fn generate_task_id() -> String {
    Uuid::new_v4().to_string()
}

impl Task {
    pub fn new(task_type: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            task_type: task_type.into(),
            payload,
            priority: Priority::Normal,
            reply_to_task: None,
            thread_id: None,
        }
    }

    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_thread(mut self, thread_id: impl Into<String>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    pub fn replying_to(mut self, task_id: impl Into<String>) -> Self {
        self.reply_to_task = Some(task_id.into());
        self
    }

    /// Convert this task into [`ConstellationMetadata`] for embedding in a message.
    pub fn to_metadata(&self) -> ConstellationMetadata {
        ConstellationMetadata {
            task_id: self.id.clone(),
            task_type: self.task_type.clone(),
            payload: self.payload.clone(),
            priority: self.priority,
            reply_to_task: self.reply_to_task.clone(),
            thread_id: self.thread_id.clone(),
        }
    }
}

/// The result of completing a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub result_data: serde_json::Value,
}

impl TaskResult {
    pub fn success(task_id: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            task_id: task_id.into(),
            status: TaskStatus::Completed,
            result_data: data,
        }
    }

    pub fn failure(task_id: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            task_id: task_id.into(),
            status: TaskStatus::Failed,
            result_data: data,
        }
    }
}

// ---------------------------------------------------------------------------
// Mention helpers
// ---------------------------------------------------------------------------

/// Extract Matrix user IDs mentioned in a message body via `@username:server` patterns.
pub fn parse_mentions(body: &str) -> Vec<String> {
    let mut mentions = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = body.chars().collect();
    while i < chars.len() {
        if chars[i] == '@' {
            let start = i;
            i += 1;
            // Consume localpart (alphanumeric, -, _, .)
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }
            // Expect ':'
            if i < chars.len() && chars[i] == ':' {
                i += 1;
                // Consume server part
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '.' || chars[i] == '-' || chars[i] == ':')
                {
                    i += 1;
                }
                let mention: String = chars[start..i].iter().collect();
                if mention.len() > 2 {
                    mentions.push(mention);
                }
            }
        } else {
            i += 1;
        }
    }
    mentions
}

/// Build an HTML-formatted mention link for a Matrix user.
///
/// Returns a tuple of (plain_text, html) suitable for Matrix message `body` and `formatted_body`.
pub fn format_mention(user_id: &str, display_name: &str) -> (String, String) {
    let plain = format!("{display_name}");
    let html = format!(
        "<a href=\"https://matrix.to/#/{user_id}\">{display_name}</a>"
    );
    (plain, html)
}

/// Build a full message body with a leading mention.
pub fn format_mention_message(
    user_id: &str,
    display_name: &str,
    message: &str,
) -> (String, String) {
    let plain = format!("{display_name} {message}");
    let html = format!(
        "<a href=\"https://matrix.to/#/{user_id}\">{display_name}</a> {message}"
    );
    (plain, html)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mentions() {
        let body = "@agent-a:constellation.local can you help @agent-b:constellation.local?";
        let mentions = parse_mentions(body);
        assert_eq!(mentions.len(), 2);
        assert_eq!(mentions[0], "@agent-a:constellation.local");
        assert_eq!(mentions[1], "@agent-b:constellation.local");
    }

    #[test]
    fn test_parse_mentions_none() {
        let mentions = parse_mentions("hello world, no mentions here");
        assert!(mentions.is_empty());
    }

    #[test]
    fn test_format_mention_message() {
        let (plain, html) = format_mention_message(
            "@agent-b:constellation.local",
            "@agent-b",
            "analyze this data",
        );
        assert_eq!(plain, "@agent-b analyze this data");
        assert!(html.contains("matrix.to"));
        assert!(html.contains("@agent-b:constellation.local"));
    }
}
