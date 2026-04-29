use anyhow::{anyhow, Result};
use constellation_a2a::{Message, Part, Role};
use constellation_client::A2aClient;
use constellation_store::{peers as peers_store, tasks_out, Store};
use std::path::Path;
use uuid::Uuid;

use crate::commands::load_config;

pub async fn run(path: &Path, peer_name: &str, text: &str) -> Result<()> {
    let cfg = load_config(path)?;
    let store = Store::open(cfg.store_path())?;
    let peers = peers_store::list_peers(&store)?;
    let peer = peers
        .iter()
        .find(|p| p.card.name == peer_name)
        .ok_or_else(|| {
            anyhow!("peer '{peer_name}' not in store. run `constellation peers` first.")
        })?;
    let task_id = format!("t-{}", Uuid::new_v4());
    let msg = Message {
        role: Role::User,
        parts: vec![Part::Text {
            text: text.to_string(),
        }],
    };
    tasks_out::insert(&store, &task_id, &peer.card.name, &msg)?;
    let client = A2aClient::new();
    let _ = client
        .send_task(peer.card.url.as_str(), &task_id, &msg)
        .await?;
    println!("{task_id}");
    Ok(())
}
