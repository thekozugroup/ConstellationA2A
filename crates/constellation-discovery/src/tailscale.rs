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

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "Self": { "TailscaleIPs": ["100.76.147.110"], "Online": true, "HostName": "atmos-vnic" },
      "Peer": {
        "abc": { "TailscaleIPs": ["100.76.147.42"], "Online": true, "HostName": "kraken" },
        "def": { "TailscaleIPs": ["100.76.147.43"], "Online": false, "HostName": "offline" }
      }
    }"#;

    #[test]
    fn parses_online_peers_only() {
        let peers = parse_status_json(FIXTURE).expect("parse");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].host, "kraken");
        assert_eq!(peers[0].ip.to_string(), "100.76.147.42");
    }

    #[test]
    fn fails_on_malformed_json() {
        let result = parse_status_json("{ malformed: json ");
        assert!(result.is_err());
    }

    #[test]
    fn ignores_peer_missing_ip() {
        let json = r#"{
          "Peer": {
            "abc": { "Online": true, "HostName": "no-ip-node" }
          }
        }"#;
        let peers = parse_status_json(json).expect("parse");
        assert_eq!(peers.len(), 0);
    }

    #[test]
    fn ignores_peer_with_invalid_ip() {
        let json = r#"{
          "Peer": {
            "abc": { "TailscaleIPs": ["not.an.ip.address"], "Online": true, "HostName": "invalid-ip-node" }
          }
        }"#;
        let peers = parse_status_json(json).expect("parse");
        assert_eq!(peers.len(), 0);
    }

    #[test]
    fn handles_missing_peer_object() {
        let json = r#"{
          "Self": { "TailscaleIPs": ["100.76.147.110"], "Online": true, "HostName": "atmos-vnic" }
        }"#;
        let peers = parse_status_json(json).expect("parse");
        assert_eq!(peers.len(), 0);
    }

    #[test]
    fn handles_missing_online_and_hostname_fields() {
        let json = r#"{
          "Peer": {
            "abc": { "TailscaleIPs": ["100.76.147.42"] }
          }
        }"#;
        // The default for `Online` is false, so this peer should be ignored.
        let peers = parse_status_json(json).expect("parse");
        assert_eq!(peers.len(), 0);
    }

    #[test]
    fn handles_missing_tailscale_ips_field() {
        let json = r#"{
          "Peer": {
            "abc": { "Online": true, "HostName": "no-ips" }
          }
        }"#;
        // The default for `TailscaleIPs` is an empty Vec, so this peer should be ignored.
        let peers = parse_status_json(json).expect("parse");
        assert_eq!(peers.len(), 0);
    }

    #[test]
    fn parses_multiple_online_peers() {
        let json = r#"{
          "Peer": {
            "abc": { "TailscaleIPs": ["100.76.147.42"], "Online": true, "HostName": "node-1" },
            "def": { "TailscaleIPs": ["100.76.147.43"], "Online": true, "HostName": "node-2" }
          }
        }"#;
        let mut peers = parse_status_json(json).expect("parse");
        assert_eq!(peers.len(), 2);

        peers.sort_by(|a, b| a.host.cmp(&b.host));

        assert_eq!(peers[0].host, "node-1");
        assert_eq!(peers[0].ip.to_string(), "100.76.147.42");
        assert_eq!(peers[1].host, "node-2");
        assert_eq!(peers[1].ip.to_string(), "100.76.147.43");
    }
}
