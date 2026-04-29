use anyhow::Result;
use constellation_store::{peers as peers_store, Store};
use std::path::Path;

use crate::commands::load_config;

pub async fn run(path: &Path, json: bool) -> Result<()> {
    let cfg = load_config(path)?;
    let store = Store::open(cfg.store_path())?;
    let list = peers_store::list_peers(&store)?;
    if json {
        let cards: Vec<_> = list.iter().map(|p| &p.card).collect();
        println!("{}", serde_json::to_string_pretty(&cards)?);
    } else {
        for p in &list {
            let skills = p
                .card
                .skills
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>()
                .join(",");
            println!("{}\t{}\t{}", p.card.name, p.card.url, skills);
        }
    }
    Ok(())
}
