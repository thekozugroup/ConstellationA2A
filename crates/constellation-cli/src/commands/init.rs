use anyhow::{Context, Result};
use std::path::Path;

use crate::config::{AgentSection, Config, NetworkSection, StoreSection};
use crate::prompt::render;

pub async fn run(
    path: &Path,
    name: Option<String>,
    skills: Option<Vec<String>>,
    port: Option<u16>,
) -> Result<()> {
    let name = name.unwrap_or_else(|| {
        std::env::var("HOSTNAME")
            .or_else(|_| hostname_from_uname())
            .unwrap_or_else(|_| "constellation-node".to_string())
    });
    let skills = skills.unwrap_or_else(|| vec!["general".to_string()]);
    let port = port.unwrap_or(7777);
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

fn hostname_from_uname() -> Result<String, std::io::Error> {
    let out = std::process::Command::new("hostname").output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
