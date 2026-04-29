pub mod card;
pub mod inbox;
pub mod init;
pub mod install_service;
pub mod peers;
pub mod respond;
pub mod send;
pub mod serve;
pub mod wait;

use crate::config::Config;
use anyhow::{Context, Result};
use constellation_a2a::{AgentCapabilities, AgentCard, Skill};
use std::net::SocketAddr;
use std::path::Path;
use url::Url;

pub fn load_config(path: &Path) -> Result<Config> {
    Config::load(path).with_context(|| format!("could not load config at {}", path.display()))
}

pub async fn build_card_from_config(cfg: &Config) -> Result<AgentCard> {
    let host = crate::net::resolve_advertised_host(&cfg.network.advertised_host).await?;
    let bind: SocketAddr = cfg
        .network
        .bind
        .parse()
        .with_context(|| format!("invalid bind address: {}", cfg.network.bind))?;
    let port = bind.port();
    let url = Url::parse(&format!("http://{host}:{port}"))?;
    let skills = cfg
        .agent
        .skills
        .iter()
        .map(|s| Skill {
            id: s.clone(),
            name: s.clone(),
            description: None,
            tags: vec![s.clone()],
        })
        .collect();
    Ok(AgentCard {
        name: cfg.agent.name.clone(),
        description: cfg.agent.description.clone(),
        url,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec!["text".into()],
        default_output_modes: vec!["text".into()],
        skills,
    })
}
