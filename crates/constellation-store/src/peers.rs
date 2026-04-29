use chrono::{DateTime, Utc};
use constellation_a2a::AgentCard;
use rusqlite::params;

use crate::{Result, Store, StoreError};

#[derive(Debug, Clone)]
pub struct PeerRecord {
    pub id: String,
    pub card: AgentCard,
    pub last_seen: DateTime<Utc>,
}

pub fn upsert_peer(store: &Store, card: &AgentCard, last_seen: DateTime<Utc>) -> Result<()> {
    let url_str = card.url.as_str();
    let card_json = serde_json::to_string(card)?;
    let last = last_seen.to_rfc3339();
    store.with_conn(|conn| {
        conn.execute(
            "INSERT INTO peers(id, name, url, card_json, last_seen) VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, url=excluded.url,
                 card_json=excluded.card_json, last_seen=excluded.last_seen",
            params![url_str, card.name, url_str, card_json, last],
        )?;
        Ok(())
    })
}

pub fn list_peers(store: &Store) -> Result<Vec<PeerRecord>> {
    store.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT id, card_json, last_seen FROM peers ORDER BY name")?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let card_json: String = row.get(1)?;
                let last_seen: String = row.get(2)?;
                Ok((id, card_json, last_seen))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (id, card_json, last_seen) in rows {
            let card: AgentCard = serde_json::from_str(&card_json)?;
            let last_seen = DateTime::parse_from_rfc3339(&last_seen)
                .map_err(|e| StoreError::Date(e.to_string()))?
                .with_timezone(&Utc);
            out.push(PeerRecord {
                id,
                card,
                last_seen,
            });
        }
        Ok(out)
    })
}

pub fn prune_older_than(store: &Store, cutoff: DateTime<Utc>) -> Result<usize> {
    store.with_conn(|conn| {
        let n = conn.execute(
            "DELETE FROM peers WHERE last_seen < ?1",
            params![cutoff.to_rfc3339()],
        )?;
        Ok(n)
    })
}
