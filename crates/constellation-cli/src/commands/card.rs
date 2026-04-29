use anyhow::Result;
use std::path::Path;

use crate::commands::{build_card_from_config, load_config};

pub async fn run(path: &Path) -> Result<()> {
    let cfg = load_config(path)?;
    let card = build_card_from_config(&cfg).await?;
    println!("{}", serde_json::to_string_pretty(&card)?);
    Ok(())
}
