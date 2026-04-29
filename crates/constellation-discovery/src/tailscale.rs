use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::IpAddr;
use std::time::Duration;

use crate::{probe::probe_card, DiscoveredPeer, Discoverer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscalePeer {
    pub host: String,
    pub ip: IpAddr,
}

#[derive(Deserialize)]
struct StatusJson {
    #[serde(rename = "Peer", default)]
    peer: std::collections::HashMap<String, NodeJson>,
}

#[derive(Deserialize)]
struct NodeJson {
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<String>,
    #[serde(rename = "Online", default)]
    online: bool,
    #[serde(rename = "HostName", default)]
    host_name: String,
}

pub fn parse_status_json(raw: &str) -> Result<Vec<TailscalePeer>> {
    let parsed: StatusJson = serde_json::from_str(raw).context("parse tailscale status")?;
    let mut out = Vec::new();
    for (_id, node) in parsed.peer {
        if !node.online {
            continue;
        }
        if let Some(ip) = node.tailscale_ips.first() {
            if let Ok(parsed) = ip.parse() {
                out.push(TailscalePeer {
                    host: node.host_name,
                    ip: parsed,
                });
            }
        }
    }
    Ok(out)
}

pub async fn fetch_status() -> Result<String> {
    let out = tokio::process::Command::new("tailscale")
        .arg("status")
        .arg("--json")
        .output()
        .await
        .context("invoke `tailscale status --json`")?;
    if !out.status.success() {
        anyhow::bail!("tailscale status exited with status {}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Debug, Clone)]
pub struct TailscaleDiscoverer {
    pub port: u16,
    pub probe_timeout: Duration,
}

impl Default for TailscaleDiscoverer {
    fn default() -> Self {
        Self {
            port: 7777,
            probe_timeout: Duration::from_secs(3),
        }
    }
}

#[async_trait::async_trait]
impl Discoverer for TailscaleDiscoverer {
    fn name(&self) -> &'static str {
        "tailscale"
    }

    async fn poll(&self) -> Vec<DiscoveredPeer> {
        let raw = match fetch_status().await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(error=%e, "tailscale status unavailable");
                return vec![];
            }
        };
        let peers = match parse_status_json(&raw) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error=%e, "could not parse tailscale status");
                return vec![];
            }
        };
        let port = self.port;
        let mut out = Vec::new();
        for peer in peers {
            let base = format!("http://{}:{port}", peer.ip);
            match probe_card(&base).await {
                Ok(card) => out.push(DiscoveredPeer {
                    host: peer.host,
                    ip: peer.ip,
                    port,
                    card,
                }),
                Err(e) => tracing::debug!(host=%peer.host, error=%e, "probe failed"),
            }
        }
        out
    }
}
