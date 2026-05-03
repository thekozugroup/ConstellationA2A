//! `constellation init` command — generate an initial config file.

use anyhow::{Context, Result};
use std::path::Path;

use crate::config::{AgentSection, Config, NetworkSection, StoreSection};
use crate::prompt::render;

/// Write a fresh config file at `path`, printing a setup prompt for an LLM coding agent.
pub async fn run(
    path: &Path,
    name: Option<String>,
    skills: Option<Vec<String>>,
    port: Option<u16>,
) -> Result<()> {
    let name = name.unwrap_or_else(|| {
        std::env::var("HOSTNAME")
            .or_else(|_| hostname::get().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|_| "constellation-node".to_string())
    });
    let skills = skills.unwrap_or_else(|| vec!["general".to_string()]);
    let port = port.unwrap_or(constellation_a2a::DEFAULT_PORT);
    let cfg = Config {
        agent: AgentSection {
            name: name.clone(),
            description: None,
            skills: skills.clone(),
        },
        network: NetworkSection {
            bind: format!("0.0.0.0:{port}"),
            advertised_host: "auto".into(),
            discovery: vec!["tailscale".into(), "mdns".into()],
        },
        store: StoreSection {
            path: "auto".into(),
        },
    };
    cfg.save(path).context("save config")?;
    let local_url = format!("http://<advertised host>:{port}");
    println!("config written: {}\n", path.display());
    println!("--- copy the prompt below into your LLM coding agent ---\n");
    println!(
        "{}",
        render(
            &name,
            &skills,
            &local_url,
            &cfg.store_path().display().to_string()
        )
    );
    Ok(())
}
