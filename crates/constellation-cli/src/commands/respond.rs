use anyhow::{anyhow, Result};
use constellation_a2a::{Message, Part, Role, TaskState};
use constellation_store::{tasks_in, Store};
use std::path::Path;

use crate::commands::load_config;

pub async fn run(path: &Path, task_id: &str, text: &str) -> Result<()> {
    let cfg = load_config(path)?;
    let store = Store::open(cfg.store_path())?;
    let _existing = tasks_in::get(&store, task_id)?
        .ok_or_else(|| anyhow!("no inbound task with id {task_id}"))?;
    let msg = Message {
        role: Role::Agent,
        parts: vec![Part::Text {
            text: text.to_string(),
        }],
    };
    tasks_in::set_response(&store, task_id, &msg, TaskState::Completed)?;
    println!("ok");
    Ok(())
}
