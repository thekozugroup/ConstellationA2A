use chrono::{DateTime, Utc};
use constellation_a2a::{Message, TaskState};
use rusqlite::params;

use crate::{Result, Store, StoreError};

#[derive(Debug, Clone)]
pub struct OutTask {
    pub task_id: String,
    pub to_peer: String,
    pub state: TaskState,
    pub request: Message,
    pub response: Option<Message>,
    pub updated_at: DateTime<Utc>,
}

pub fn insert(store: &Store, task_id: &str, to_peer: &str, request: &Message) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let req_json = serde_json::to_string(request)?;
    store.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO tasks_out(task_id, to_peer, state, request_json, created_at, updated_at)
             VALUES (?1,?2,'submitted',?3,?4,?4)",
            params![task_id, to_peer, req_json, now],
        )?;
        Ok(())
    })
}

pub fn set_response(
    store: &Store,
    task_id: &str,
    response: &Message,
    state: TaskState,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let resp_json = serde_json::to_string(response)?;
    store.with_conn(|conn| {
        conn.execute(
            "UPDATE tasks_out SET response_json=?1, state=?2, updated_at=?3 WHERE task_id=?4",
            params![resp_json, state.as_str(), now, task_id],
        )?;
        Ok(())
    })
}

pub fn set_state(store: &Store, task_id: &str, state: TaskState) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    store.with_conn(|conn| {
        conn.execute(
            "UPDATE tasks_out SET state=?1, updated_at=?2 WHERE task_id=?3",
            params![state.as_str(), now, task_id],
        )?;
        Ok(())
    })
}

pub fn get(store: &Store, task_id: &str) -> Result<Option<OutTask>> {
    store.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT task_id, to_peer, state, request_json, response_json, updated_at FROM tasks_out WHERE task_id=?1",
        )?;
        let mut rows = stmt.query(params![task_id])?;
        if let Some(row) = rows.next()? {
            let task_id: String = row.get(0)?;
            let to_peer: String = row.get(1)?;
            let state: String = row.get(2)?;
            let request_json: String = row.get(3)?;
            let response_json: Option<String> = row.get(4)?;
            let updated_at_str: String = row.get(5)?;
            let request: Message = serde_json::from_str(&request_json)?;
            let response = response_json
                .as_deref()
                .map(serde_json::from_str::<Message>)
                .transpose()?;
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| StoreError::Date(e.to_string()))?
                .with_timezone(&Utc);
            Ok(Some(OutTask {
                task_id,
                to_peer,
                state: TaskState::parse(&state),
                request,
                response,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    })
}
