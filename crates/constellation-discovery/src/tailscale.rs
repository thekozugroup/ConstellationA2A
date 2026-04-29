//! Tailscale-based peer discoverer.

use anyhow::{Context, Result};
use futures_util::stream::{FuturesUnordered, StreamExt};
use serde::Deserialize;
use std::net::IpAddr;
use std::time::Duration;

use crate::{probe::probe_card, DiscoveredPeer, Discoverer};

/// A Tailscale peer that is currently online.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscalePeer {
    /// Hostname reported by Tailscale.
    pub host: String,
    /// Primary Tailscale IP address.
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

/// Parse the JSON output of `tailscale status --json` into a list of online peers.
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

/// Invoke `tailscale status --json` and return its raw output.
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

/// Discovers A2A peers reachable on the local Tailscale network.
#[derive(Clone)]
pub struct TailscaleDiscoverer {
    /// Port to probe on each peer.
    port: u16,
    probe_timeout: Duration,
    client: reqwest::Client,
}

impl Default for TailscaleDiscoverer {
    fn default() -> Self {
        Self::new(constellation_a2a::DEFAULT_PORT, Duration::from_secs(3))
    }
}

impl TailscaleDiscoverer {
    /// Create a new discoverer that probes the given `port` with the given `probe_timeout`.
    pub fn new(port: u16, probe_timeout: Duration) -> Self {
        Self {
            port,
            probe_timeout,
            client: crate::probe::default_client(probe_timeout),
        }
    }

    /// Return the configured per-probe HTTP timeout.
    pub fn probe_timeout(&self) -> Duration {
        self.probe_timeout
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
        let client = &self.client;
        let mut tasks: FuturesUnordered<_> = peers
            .into_iter()
            .map(|peer| async move {
                let base = format!("http://{}:{port}", peer.ip);
                match probe_card(client, &base).await {
                    Ok(card) => Some(DiscoveredPeer {
                        host: peer.host,
                        ip: peer.ip,
                        port,
                        card,
                    }),
                    Err(e) => {
                        tracing::debug!(host=%peer.host, error=%e, "probe failed");
                        None
                    }
                }
            })
            .collect();
        let mut out = Vec::new();
        while let Some(maybe) = tasks.next().await {
            if let Some(p) = maybe {
                out.push(p);
            }
        }
        out
    }
}
