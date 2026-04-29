use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub url: Url,
    pub version: String,
    #[serde(default)]
    pub capabilities: AgentCapabilities,
    #[serde(rename = "defaultInputModes", default = "default_modes_text")]
    pub default_input_modes: Vec<String>,
    #[serde(rename = "defaultOutputModes", default = "default_modes_text")]
    pub default_output_modes: Vec<String>,
    #[serde(default)]
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilities {
    #[serde(default)]
    pub streaming: bool,
    #[serde(default, rename = "pushNotifications")]
    pub push_notifications: bool,
    #[serde(default, rename = "stateTransitionHistory")]
    pub state_transition_history: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_modes_text() -> Vec<String> {
    vec!["text".into()]
}
