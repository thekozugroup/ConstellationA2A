//! Peer discovery — Tailscale and mDNS implementations.

pub mod mdns;
pub mod probe;
pub mod tailscale;

use constellation_a2a::AgentCard;
use std::net::IpAddr;

/// A remote A2A peer that was found and successfully probed.
#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    /// Hostname of the peer.
    pub host: String,
    /// IP address used to reach the peer.
    pub ip: IpAddr,
    /// Port on which the peer's A2A endpoint is listening.
    pub port: u16,
    /// Agent card returned by the peer's `/.well-known/agent.json` endpoint.
    pub card: AgentCard,
}

/// Trait implemented by each discovery backend.
#[async_trait::async_trait]
pub trait Discoverer: Send + Sync {
    /// Poll for currently reachable peers.
    async fn poll(&self) -> Vec<DiscoveredPeer>;
    /// Short name of this discoverer (e.g. `"tailscale"`, `"mdns"`).
    fn name(&self) -> &'static str;
}
