/// DDL statements that initialize all tables and indexes.
pub const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS peers (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    url             TEXT NOT NULL,
    card_json       TEXT NOT NULL,
    last_seen       TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks_in (
    task_id         TEXT PRIMARY KEY,
    from_peer       TEXT NOT NULL,
    state           TEXT NOT NULL,
    request_json    TEXT NOT NULL,
    response_json   TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS tasks_in_state_idx ON tasks_in(state);

CREATE TABLE IF NOT EXISTS tasks_out (
    task_id         TEXT PRIMARY KEY,
    to_peer         TEXT NOT NULL,
    state           TEXT NOT NULL,
    request_json    TEXT NOT NULL,
    response_json   TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS tasks_out_state_idx ON tasks_out(state);
"#;
