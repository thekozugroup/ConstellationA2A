use anyhow::{anyhow, Result};
use constellation_a2a::AgentCard;
use std::time::Duration;

pub async fn probe_card(base_url: &str) -> Result<AgentCard> {
    let url = format!("{}/.well-known/agent.json", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("probe returned status {}", resp.status()));
    }
    let card: AgentCard = resp.json().await?;
    Ok(card)
}
