use serde::{Deserialize, Serialize};
use url::Url;

/// Describes an A2A agent — its identity, endpoint, capabilities, and skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCard {
    /// Unique human-readable name of the agent.
    pub name: String,
    /// Optional prose description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Base URL at which the agent's JSON-RPC endpoint is reachable.
    pub url: Url,
    /// Semantic version string of the agent implementation.
    pub version: String,
    /// Feature flags for optional A2A protocol capabilities.
    #[serde(default)]
    pub capabilities: AgentCapabilities,
    /// MIME-like input mode identifiers the agent accepts by default.
    #[serde(rename = "defaultInputModes", default = "default_modes_text")]
    pub default_input_modes: Vec<String>,
    /// MIME-like output mode identifiers the agent produces by default.
    #[serde(rename = "defaultOutputModes", default = "default_modes_text")]
    pub default_output_modes: Vec<String>,
    /// Skills this agent advertises.
    #[serde(default)]
    pub skills: Vec<Skill>,
}

/// Optional protocol capability flags for an agent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// Whether the agent supports server-sent event streaming.
    #[serde(default)]
    pub streaming: bool,
    /// Whether the agent supports push-notification callbacks.
    #[serde(default, rename = "pushNotifications")]
    pub push_notifications: bool,
    /// Whether the agent records state-transition history.
    #[serde(default, rename = "stateTransitionHistory")]
    pub state_transition_history: bool,
}

/// A named capability unit that an agent can advertise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    /// Stable identifier for the skill.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Optional prose description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Searchable tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_modes_text() -> Vec<String> {
    vec!["text".into()]
}
