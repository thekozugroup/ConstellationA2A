use crate::error::{ConstellationError, Result};

/// Configuration for a Constellation agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub homeserver_url: String,
    pub username: String,
    pub password: String,
    pub display_name: Option<String>,
    pub auto_join_rooms: Vec<String>,
    pub device_id: Option<String>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            homeserver_url: "http://localhost:6167".to_string(),
            username: String::new(),
            password: String::new(),
            display_name: None,
            auto_join_rooms: Vec::new(),
            device_id: None,
        }
    }
}

impl AgentConfig {
    /// Validate that required fields are set.
    pub fn validate(&self) -> Result<()> {
        if self.username.is_empty() {
            return Err(ConstellationError::Config(
                "username is required".to_string(),
            ));
        }
        if self.password.is_empty() {
            return Err(ConstellationError::Config(
                "password is required".to_string(),
            ));
        }
        if self.homeserver_url.is_empty() {
            return Err(ConstellationError::Config(
                "homeserver_url is required".to_string(),
            ));
        }
        Ok(())
    }
}

/// Builder for constructing an [`AgentConfig`] step by step.
pub struct AgentConfigBuilder {
    config: AgentConfig,
}

impl AgentConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: AgentConfig::default(),
        }
    }

    pub fn homeserver_url(mut self, url: impl Into<String>) -> Self {
        self.config.homeserver_url = url.into();
        self
    }

    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.config.username = username.into();
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.config.password = password.into();
        self
    }

    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.config.display_name = Some(name.into());
        self
    }

    pub fn auto_join_room(mut self, room: impl Into<String>) -> Self {
        self.config.auto_join_rooms.push(room.into());
        self
    }

    pub fn auto_join_rooms(mut self, rooms: Vec<String>) -> Self {
        self.config.auto_join_rooms = rooms;
        self
    }

    pub fn device_id(mut self, id: impl Into<String>) -> Self {
        self.config.device_id = Some(id.into());
        self
    }

    pub fn build(self) -> Result<AgentConfig> {
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for AgentConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
