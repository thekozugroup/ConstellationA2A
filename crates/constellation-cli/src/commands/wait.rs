use anyhow::{anyhow, Result};
use constellation_a2a::TaskState;
use constellation_client::A2aClient;
use constellation_store::{peers as peers_store, tasks_out, Store};
use std::{path::Path, time::Duration};

use crate::commands::{build_card_from_config, load_config};

/// Poll a remote peer until `task_id` reaches a terminal state or timeout.
pub async fn run(path: &Path, task_id: &str, timeout_secs: u64) -> Result<()> {
    let cfg = load_config(path)?;
    let card = build_card_from_config(&cfg).await?;
    let store = Store::open(cfg.store_path())?;
    let task = tasks_out::get(&store, task_id)?
        .ok_or_else(|| anyhow!("no outbound task with id {task_id}"))?;
    let peers = peers_store::list_peers(&store)?;
    let peer = peers
        .iter()
        .find(|p| p.card.name == task.to_peer)
        .ok_or_else(|| anyhow!("peer '{}' not in store", task.to_peer))?;
    let client = A2aClient::new().with_source_url(card.url.as_str());
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let result = client.get_task(peer.card.url.as_str(), task_id).await?;
        match result.status.state {
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                if let Some(last) = result.history.last() {
                    let body = last
                        .parts
                        .iter()
                        .map(|p| match p {
                            constellation_a2a::Part::Text { text } => text.as_str(),
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    println!("{body}");
                    tasks_out::set_response(&store, task_id, last, result.status.state.clone())?;
                } else {
                    tasks_out::set_state(&store, task_id, result.status.state)?;
                }
                return Ok(());
            }
            _ => {
                if std::time::Instant::now() >= deadline {
                    return Err(anyhow!("timed out waiting for task {task_id}"));
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
