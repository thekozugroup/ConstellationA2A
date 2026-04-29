use chrono::{DateTime, Utc};
use constellation_a2a::{Message, TaskState};
use rusqlite::params;

use crate::{Result, Store, StoreError};

#[derive(Debug, Clone)]
pub struct InTask {
    pub task_id: String,
    pub from_peer: String,
    pub state: TaskState,
    pub request: Message,
    pub response: Option<Message>,
    pub updated_at: DateTime<Utc>,
}

pub fn insert(store: &Store, task_id: &str, from_peer: &str, request: &Message) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let req_json = serde_json::to_string(request)?;
    store.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO tasks_in(task_id, from_peer, state, request_json, created_at, updated_at)
             VALUES (?1,?2,'submitted',?3,?4,?4)",
            params![task_id, from_peer, req_json, now],
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
            "UPDATE tasks_in SET response_json=?1, state=?2, updated_at=?3 WHERE task_id=?4",
            params![resp_json, state.as_str(), now, task_id],
        )?;
        Ok(())
    })
}

pub fn get(store: &Store, task_id: &str) -> Result<Option<InTask>> {
    store.with_conn(|conn| {
        let mut stmt = conn.prepare(
            "SELECT task_id, from_peer, state, request_json, response_json, updated_at FROM tasks_in WHERE task_id=?1",
        )?;
        let mut rows = stmt.query(params![task_id])?;
        if let Some(row) = rows.next()? {
            let task_id: String = row.get(0)?;
            let from_peer: String = row.get(1)?;
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
            Ok(Some(InTask {
                task_id,
                from_peer,
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

pub fn list_pending(store: &Store) -> Result<Vec<InTask>> {
    list_with_states(store, &["submitted", "working", "input-required"])
}

fn list_with_states(store: &Store, states: &[&str]) -> Result<Vec<InTask>> {
    store.with_conn(|conn| {
        let placeholders = states.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT task_id, from_peer, state, request_json, response_json, updated_at
             FROM tasks_in WHERE state IN ({placeholders}) ORDER BY created_at"
        );
        let mut stmt = conn.prepare(&sql)?;
        let params_iter = rusqlite::params_from_iter(states.iter());
        let rows = stmt
            .query_map(params_iter, |row| {
                let task_id: String = row.get(0)?;
                let from_peer: String = row.get(1)?;
                let state: String = row.get(2)?;
                let request_json: String = row.get(3)?;
                let response_json: Option<String> = row.get(4)?;
                let updated_at_str: String = row.get(5)?;
                Ok((
                    task_id,
                    from_peer,
                    state,
                    request_json,
                    response_json,
                    updated_at_str,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (task_id, from_peer, state, request_json, response_json, updated_at_str) in rows {
            let request: Message = serde_json::from_str(&request_json)?;
            let response = response_json
                .as_deref()
                .map(serde_json::from_str::<Message>)
                .transpose()?;
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map_err(|e| StoreError::Date(e.to_string()))?
                .with_timezone(&Utc);
            out.push(InTask {
                task_id,
                from_peer,
                state: TaskState::parse(&state),
                request,
                response,
                updated_at,
            });
        }
        Ok(out)
    })
}
