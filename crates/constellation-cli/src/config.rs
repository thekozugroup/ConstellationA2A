use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agent: AgentSection,
    pub network: NetworkSection,
    pub store: StoreSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSection {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSection {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_advertised")]
    pub advertised_host: String,
    #[serde(default = "default_discovery")]
    pub discovery: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSection {
    #[serde(default = "default_store")]
    pub path: String,
}

fn default_bind() -> String {
    "0.0.0.0:7777".into()
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
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("constellation/config.toml")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

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
