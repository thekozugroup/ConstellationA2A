use anyhow::Result;
use constellation_store::{tasks_in, Store};
use std::path::Path;

use crate::commands::load_config;

pub async fn run(path: &Path, json: bool) -> Result<()> {
    let cfg = load_config(path)?;
    let store = Store::open(cfg.store_path())?;
    let pending = tasks_in::list_pending(&store)?;
    if json {
        let view: Vec<_> = pending
            .iter()
            .map(|t| {
                serde_json::json!({
                    "task_id": t.task_id,
                    "from_peer": t.from_peer,
                    "request": t.request,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        for t in &pending {
            let preview = t
                .request
                .parts
                .iter()
                .map(|p| match p {
                    constellation_a2a::Part::Text { text } => text.as_str(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            let preview = preview.chars().take(80).collect::<String>();
            println!("{}\t{}\t{}", t.task_id, t.from_peer, preview);
        }
    }
    Ok(())
}
