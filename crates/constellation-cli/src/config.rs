//! Configuration types loaded from the TOML config file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level configuration for a Constellation node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Agent identity settings.
    pub agent: AgentSection,
    /// Network bind and discovery settings.
    pub network: NetworkSection,
    /// Persistent store settings.
    pub store: StoreSection,
}

/// Agent identity section of the config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSection {
    /// Human-readable agent name, also used as the card identifier.
    pub name: String,
    /// Optional prose description advertised in the agent card.
    #[serde(default)]
    pub description: Option<String>,
    /// Skill IDs this agent advertises.
    #[serde(default)]
    pub skills: Vec<String>,
}

/// Network section of the config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSection {
    /// Socket address to bind the HTTP listener to.
    #[serde(default = "default_bind")]
    pub bind: String,
    /// IP or hostname to advertise; `"auto"` means auto-detect.
    #[serde(default = "default_advertised")]
    pub advertised_host: String,
    /// Ordered list of discovery backends to enable.
    #[serde(default = "default_discovery")]
    pub discovery: Vec<String>,
}

/// Store section of the config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSection {
    /// Path to the SQLite database; `"auto"` uses the platform data directory.
    #[serde(default = "default_store")]
    pub path: String,
}

fn default_bind() -> String {
    format!("0.0.0.0:{}", constellation_a2a::DEFAULT_PORT)
}

fn default_advertised() -> String {
    "auto".into()
}

fn default_discovery() -> Vec<String> {
    vec!["tailscale".into(), "mdns".into()]
}

fn default_store() -> String {
    "auto".into()
}

impl Config {
    /// Return the platform default path for the config file.
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("constellation/config.toml")
    }

    /// Load a `Config` from a TOML file at `path`.
    pub fn load(path: &Path) -> Result<Self> {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    }

    /// Serialize this config to TOML and write it to `path`, creating parent dirs.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    /// Resolve the absolute path to the SQLite store.
    pub fn store_path(&self) -> PathBuf {
        if self.store.path == "auto" {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("constellation/store.db")
        } else {
            PathBuf::from(&self.store.path)
        }
    }
}
