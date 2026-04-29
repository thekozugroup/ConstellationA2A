//! Peer discovery — Tailscale and mDNS implementations.

pub mod mdns;
pub mod probe;
pub mod tailscale;

use constellation_a2a::AgentCard;
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub host: String,
    pub ip: IpAddr,
    pub port: u16,
    pub card: AgentCard,
}

#[async_trait::async_trait]
pub trait Discoverer: Send + Sync {
    async fn poll(&self) -> Vec<DiscoveredPeer>;
    fn name(&self) -> &'static str;
}
