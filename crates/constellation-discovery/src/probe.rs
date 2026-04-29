use anyhow::{anyhow, Result};
use constellation_a2a::AgentCard;
use reqwest::Client;
use std::time::Duration;

pub fn default_client(timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .build()
        .expect("reqwest client builds")
}

pub async fn probe_card(client: &Client, base_url: &str) -> Result<AgentCard> {
    let url = format!("{}/.well-known/agent.json", base_url.trim_end_matches('/'));
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("probe returned status {}", resp.status()));
    }
    let card: AgentCard = resp.json().await?;
    Ok(card)
}
