//! SQLite persistence for Constellation peers and tasks.

pub mod peers;
mod schema;
pub mod tasks_in;
pub mod tasks_out;

use rusqlite::Connection;
use std::{path::Path, sync::Mutex};
use thiserror::Error;

/// Errors that can occur in store operations.
#[derive(Error, Debug)]
pub enum StoreError {
    /// Underlying SQLite error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// JSON serialization or deserialization error.
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    /// RFC-3339 date parse error.
    #[error("date parse error: {0}")]
    Date(String),
    /// Filesystem I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// The internal mutex was poisoned.
    #[error("lock poisoned")]
    LockPoisoned,
}

/// Convenience alias for store operation results.
pub type Result<T> = std::result::Result<T, StoreError>;

/// Thread-safe handle to the SQLite database.
pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    /// Open (or create) the SQLite database at `path`, running schema migrations.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch(schema::SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Execute `f` with exclusive access to the underlying connection.
    pub(crate) fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let guard = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        f(&guard)
    }
}
