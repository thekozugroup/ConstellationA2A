# Constellation A2A — Rust P2P Mesh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the existing Matrix-based Constellation with a single Rust binary that turns any LLM coding agent into a peer on a private A2A mesh discovered over Tailscale and mDNS.

**Architecture:** Cargo workspace with six small crates: `constellation-a2a` (wire types), `constellation-store` (sqlite), `constellation-discovery` (tailscale + mdns), `constellation-server` (axum HTTP + JSON-RPC), `constellation-client` (reqwest), `constellation-cli` (clap binary). The `serve` subcommand runs the HTTP server and discovery loop. The other CLI verbs are how the LLM agent on the device sends, receives, and answers tasks.

**Tech Stack:** Rust 1.75+, axum, tokio, reqwest, clap (derive), serde, rusqlite (bundled), mdns-sd, anyhow, thiserror, tracing, tracing-subscriber. Test stack: `tempfile`, `wiremock` for client tests, `tokio::test`, fixture JSON for spec conformance.

---

## File Structure

```
ConstellationA2A/
  Cargo.toml                                # workspace
  rust-toolchain.toml                       # pin stable
  rustfmt.toml                              # formatting
  .github/workflows/ci.yml                  # rewritten CI
  README.md                                 # rewritten
  docs/
    setup-prompt.md                         # embedded prompt template
    SECURITY.md                             # trust model
    superpowers/
      specs/2026-04-29-constellation-rust-a2a-design.md  # exists
      plans/2026-04-29-constellation-rust-a2a.md         # this file
  scripts/
    health-check.sh                         # rewritten
    security-check.sh                       # rewritten
  crates/
    constellation-a2a/
      Cargo.toml
      src/
        lib.rs                              # re-exports
        card.rs                             # AgentCard, Skill
        task.rs                             # Task, TaskState, Message, Part
        rpc.rs                              # JsonRpcRequest/Response, method enum
        error.rs                            # JsonRpcError, ErrorCode
      tests/
        fixtures/
          agent_card.json
          tasks_send_request.json
          tasks_get_response.json
        conformance.rs                      # round-trip tests
    constellation-store/
      Cargo.toml
      src/
        lib.rs
        schema.rs                           # CREATE TABLE statements
        peers.rs                            # upsert_peer, list_peers, prune_stale
        tasks_in.rs                         # insert_in_task, set_in_response, ...
        tasks_out.rs                        # insert_out_task, set_out_response, ...
      tests/
        store_tests.rs                      # sqlite-in-tmpdir round-trips
    constellation-discovery/
      Cargo.toml
      src/
        lib.rs                              # Discoverer trait, DiscoveredPeer
        tailscale.rs                        # TailscaleDiscoverer
        mdns.rs                             # MdnsDiscoverer
        probe.rs                            # GET /.well-known/agent.json
      tests/
        probe_tests.rs                      # spawns wiremock, validates probe
        tailscale_parse_tests.rs            # parses fixture json
    constellation-server/
      Cargo.toml
      src/
        lib.rs                              # router builder
        rpc.rs                              # tasks/send|get|cancel handlers
        well_known.rs                       # GET /.well-known/agent.json
        state.rs                            # AppState (db handle + card)
      tests/
        rpc_tests.rs                        # axum::test_helpers
    constellation-client/
      Cargo.toml
      src/
        lib.rs                              # send_task, get_task, cancel_task
      tests/
        client_tests.rs                     # wiremock round-trip
    constellation-cli/
      Cargo.toml                            # produces `constellation` binary
      src/
        main.rs                             # clap dispatch
        config.rs                           # AgentConfig load/save
        commands/
          init.rs
          serve.rs
          peers.rs
          send.rs
          wait.rs
          inbox.rs
          respond.rs
          card.rs
          install_service.rs
        net.rs                              # advertised_host resolver
        prompt.rs                           # include_str!("../../../docs/setup-prompt.md")
      assets/
        constellation.service.tmpl
      tests/
        cli_smoke.rs                        # `constellation --help` smoke
  tests/
    integration/
      two_peer_loopback.rs                  # spawns two `serve` processes
```

---

## Parallelisation Map (for subagent-driven execution)

The dependency graph permits four execution waves. Inside a wave, all tasks are independent and can be dispatched concurrently to subagents.

| Wave | Tasks                                                                                                         | Notes                                                                            |
| ---- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| 0    | T1 (workspace skeleton + strip)                                                                               | Must complete first — every later task needs the workspace.                      |
| 1    | T2 (a2a types), T3 (store), T4 (setup-prompt + docs)                                                          | All independent of each other. Foundation for wave 2.                            |
| 2    | T5 (discovery probe + tailscale + mdns), T6 (client), T7 (server)                                             | All depend on T2 only. Independent of each other.                                |
| 3    | T8 (cli — config / serve / verbs)                                                                             | Depends on T2, T3, T5, T6, T7.                                                   |
| 4    | T9 (integration test), T10 (CI + scripts), T11 (README rewrite)                                               | Depend on T8.                                                                    |

Subagent-driven execution is the recommended workflow. Each task is self-contained: a fresh subagent can complete it end-to-end (write tests, implement, run tests, commit). The model `claude-sonnet-4-6` is appropriate for these tasks.

---

## Task 1: Workspace skeleton and strip legacy files

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`
- Create: `.gitignore` additions
- Delete: `conduit/`, `agents/`, `cli/constellation_cli.py`, `examples/simple_agent.py`, `examples/multi_agent_demo.py`, `sdk/constellation-core/`, `sdk/constellation-py/`, `sdk/Cargo.toml`, `sdk/rustfmt.toml`, `docker-compose.yml`, `docker-compose.prod.yml`, `.dockerignore`, `scripts/setup.sh`, `scripts/register-agents.sh`, `Makefile`, `tests/integration/`

- [ ] **Step 1: Delete obsolete files and directories**

```bash
cd ~/ConstellationA2A
git rm -r conduit agents cli examples sdk \
          docker-compose.yml docker-compose.prod.yml .dockerignore \
          scripts/setup.sh scripts/register-agents.sh \
          Makefile tests/integration
```

If `tests/` is now empty, leave it — Task 9 repopulates it.

- [ ] **Step 2: Write workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
    "crates/constellation-a2a",
    "crates/constellation-store",
    "crates/constellation-discovery",
    "crates/constellation-server",
    "crates/constellation-client",
    "crates/constellation-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["Tachyon Labs HQ <thekozugroup@gmail.com>"]
repository = "https://github.com/thekozugroup/ConstellationA2A"
rust-version = "1.75"

[workspace.dependencies]
anyhow = "1"
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "fs", "process", "sync", "time", "signal"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
axum = { version = "0.7", features = ["json", "macros"] }
tower = "0.5"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
rusqlite = { version = "0.31", features = ["bundled"] }
mdns-sd = "0.11"
clap = { version = "4", features = ["derive", "env"] }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
url = { version = "2", features = ["serde"] }
toml = "0.8"
dirs = "5"
once_cell = "1"
tempfile = "3"
wiremock = "0.6"
```

- [ ] **Step 3: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 4: Write `rustfmt.toml`**

```toml
edition = "2021"
max_width = 100
```

- [ ] **Step 5: Append to `.gitignore`**

Add the following lines to the existing `.gitignore`:

```
target/
**/*.rs.bk
Cargo.lock.bak
*.sqlite
*.sqlite-journal
.constellation/
```

- [ ] **Step 6: Verify workspace compiles empty**

```bash
mkdir -p crates && cargo metadata --format-version 1 --no-deps > /dev/null 2>&1 || true
```

Workspace will fail until Task 2 lands a member. That's expected.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "chore: strip Matrix stack and scaffold Rust workspace"
```

---

## Task 2: `constellation-a2a` — A2A wire types

**Files:**
- Create: `crates/constellation-a2a/Cargo.toml`
- Create: `crates/constellation-a2a/src/lib.rs`
- Create: `crates/constellation-a2a/src/card.rs`
- Create: `crates/constellation-a2a/src/task.rs`
- Create: `crates/constellation-a2a/src/rpc.rs`
- Create: `crates/constellation-a2a/src/error.rs`
- Create: `crates/constellation-a2a/tests/fixtures/agent_card.json`
- Create: `crates/constellation-a2a/tests/fixtures/tasks_send_request.json`
- Create: `crates/constellation-a2a/tests/fixtures/tasks_get_response.json`
- Create: `crates/constellation-a2a/tests/conformance.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "constellation-a2a"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "A2A protocol wire types for Constellation"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
url = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 2: Write fixtures (copy verbatim)**

`crates/constellation-a2a/tests/fixtures/agent_card.json`:

```json
{
  "name": "atmos-vnic",
  "description": "Cloud ARM dev box",
  "url": "http://100.76.147.110:7777",
  "version": "0.1.0",
  "capabilities": {
    "streaming": false,
    "pushNotifications": false,
    "stateTransitionHistory": false
  },
  "defaultInputModes": ["text"],
  "defaultOutputModes": ["text"],
  "skills": [
    {
      "id": "research",
      "name": "research",
      "description": "Web research and summarization",
      "tags": ["research", "web"]
    }
  ]
}
```

`crates/constellation-a2a/tests/fixtures/tasks_send_request.json`:

```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "tasks/send",
  "params": {
    "id": "task-abc",
    "message": {
      "role": "user",
      "parts": [{ "type": "text", "text": "research Cedric the shrimp" }]
    }
  }
}
```

`crates/constellation-a2a/tests/fixtures/tasks_get_response.json`:

```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "result": {
    "id": "task-abc",
    "status": {
      "state": "completed",
      "timestamp": "2026-04-29T18:00:00Z"
    },
    "history": [
      { "role": "user", "parts": [{ "type": "text", "text": "research Cedric" }] },
      { "role": "agent", "parts": [{ "type": "text", "text": "Cedric is a shrimp." }] }
    ]
  }
}
```

- [ ] **Step 3: Write the failing conformance test**

`crates/constellation-a2a/tests/conformance.rs`:

```rust
use constellation_a2a::{AgentCard, JsonRpcRequest, JsonRpcResponse, TaskGetResult};

#[test]
fn agent_card_round_trip() {
    let raw = include_str!("fixtures/agent_card.json");
    let card: AgentCard = serde_json::from_str(raw).expect("deserialize agent card");
    let again = serde_json::to_string(&card).expect("serialize");
    let reparsed: AgentCard = serde_json::from_str(&again).expect("re-parse");
    assert_eq!(card, reparsed);
    assert_eq!(card.name, "atmos-vnic");
    assert_eq!(card.skills.len(), 1);
    assert_eq!(card.skills[0].id, "research");
}

#[test]
fn tasks_send_request_round_trip() {
    let raw = include_str!("fixtures/tasks_send_request.json");
    let req: JsonRpcRequest = serde_json::from_str(raw).expect("deserialize request");
    assert_eq!(req.method, "tasks/send");
    let again = serde_json::to_string(&req).expect("serialize");
    let reparsed: JsonRpcRequest = serde_json::from_str(&again).expect("re-parse");
    assert_eq!(req, reparsed);
}

#[test]
fn tasks_get_response_round_trip() {
    let raw = include_str!("fixtures/tasks_get_response.json");
    let resp: JsonRpcResponse<TaskGetResult> =
        serde_json::from_str(raw).expect("deserialize response");
    assert!(resp.error.is_none());
    let result = resp.result.expect("result");
    assert_eq!(result.id, "task-abc");
    assert_eq!(result.status.state.as_str(), "completed");
    assert_eq!(result.history.len(), 2);
}
```

- [ ] **Step 4: Run the test (expect compile failure)**

```bash
cargo test -p constellation-a2a
```

Expected: error `cannot find type AgentCard`. Continue.

- [ ] **Step 5: Implement `error.rs`**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    pub fn parse_error() -> Self {
        Self { code: -32700, message: "Parse error".into(), data: None }
    }
    pub fn invalid_request() -> Self {
        Self { code: -32600, message: "Invalid Request".into(), data: None }
    }
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method not found: {method}"),
            data: None,
        }
    }
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self { code: -32602, message: message.into(), data: None }
    }
    pub fn internal_error(message: impl Into<String>) -> Self {
        Self { code: -32603, message: message.into(), data: None }
    }
    pub fn task_not_found(id: &str) -> Self {
        Self {
            code: -32001,
            message: format!("Task not found: {id}"),
            data: None,
        }
    }
}
```

- [ ] **Step 6: Implement `card.rs`**

```rust
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub url: Url,
    pub version: String,
    #[serde(default)]
    pub capabilities: AgentCapabilities,
    #[serde(rename = "defaultInputModes", default = "default_modes_text")]
    pub default_input_modes: Vec<String>,
    #[serde(rename = "defaultOutputModes", default = "default_modes_text")]
    pub default_output_modes: Vec<String>,
    #[serde(default)]
    pub skills: Vec<Skill>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentCapabilities {
    #[serde(default)]
    pub streaming: bool,
    #[serde(default, rename = "pushNotifications")]
    pub push_notifications: bool,
    #[serde(default, rename = "stateTransitionHistory")]
    pub state_transition_history: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_modes_text() -> Vec<String> {
    vec!["text".into()]
}
```

- [ ] **Step 7: Implement `task.rs`**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Part {
    Text { text: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    Submitted,
    Working,
    InputRequired,
    Completed,
    Canceled,
    Failed,
    Unknown,
}

impl TaskState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Working => "working",
            Self::InputRequired => "input-required",
            Self::Completed => "completed",
            Self::Canceled => "canceled",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskStatus {
    pub state: TaskState,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSendParams {
    pub id: String,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGetParams {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskGetResult {
    pub id: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub history: Vec<Message>,
}

pub type TaskSendResult = TaskGetResult;
```

- [ ] **Step 8: Implement `rpc.rs`**

```rust
use serde::{Deserialize, Serialize};
use crate::error::JsonRpcError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: JsonRpcVersion,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonRpcVersion(String);

impl Default for JsonRpcVersion {
    fn default() -> Self { Self("2.0".into()) }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcResponse<T> {
    pub jsonrpc: JsonRpcVersion,
    pub id: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl<T> JsonRpcResponse<T> {
    pub fn ok(id: serde_json::Value, result: T) -> Self {
        Self { jsonrpc: JsonRpcVersion::default(), id, result: Some(result), error: None }
    }
    pub fn err(id: serde_json::Value, error: JsonRpcError) -> Self {
        Self { jsonrpc: JsonRpcVersion::default(), id, result: None, error: Some(error) }
    }
}
```

- [ ] **Step 9: Implement `lib.rs`**

```rust
//! Constellation A2A wire types — pure (de)serialization, no I/O.

pub mod card;
pub mod error;
pub mod rpc;
pub mod task;

pub use card::{AgentCapabilities, AgentCard, Skill};
pub use error::JsonRpcError;
pub use rpc::{JsonRpcRequest, JsonRpcResponse, JsonRpcVersion};
pub use task::{
    Message, Part, Role, TaskGetParams, TaskGetResult, TaskSendParams, TaskSendResult, TaskState,
    TaskStatus,
};
```

- [ ] **Step 10: Run tests and verify they pass**

```bash
cargo test -p constellation-a2a
```

Expected: 3 passed.

- [ ] **Step 11: Commit**

```bash
git add crates/constellation-a2a
git commit -m "feat(a2a): add wire types with conformance tests"
```

---

## Task 3: `constellation-store` — sqlite persistence

**Files:**
- Create: `crates/constellation-store/Cargo.toml`
- Create: `crates/constellation-store/src/lib.rs`
- Create: `crates/constellation-store/src/schema.rs`
- Create: `crates/constellation-store/src/peers.rs`
- Create: `crates/constellation-store/src/tasks_in.rs`
- Create: `crates/constellation-store/src/tasks_out.rs`
- Create: `crates/constellation-store/tests/store_tests.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "constellation-store"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
rusqlite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
chrono = { workspace = true }
constellation-a2a = { path = "../constellation-a2a" }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Write the failing tests**

`crates/constellation-store/tests/store_tests.rs`:

```rust
use chrono::Utc;
use constellation_a2a::{AgentCard, AgentCapabilities, Message, Part, Role, Skill, TaskState};
use constellation_store::{peers, tasks_in, tasks_out, Store};
use tempfile::tempdir;
use url::Url;

fn sample_card() -> AgentCard {
    AgentCard {
        name: "test-peer".into(),
        description: Some("test".into()),
        url: Url::parse("http://10.0.0.5:7777").unwrap(),
        version: "0.1.0".into(),
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec!["text".into()],
        default_output_modes: vec!["text".into()],
        skills: vec![Skill { id: "x".into(), name: "x".into(), description: None, tags: vec![] }],
    }
}

#[test]
fn store_initializes_schema() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    assert!(peers::list_peers(&store).unwrap().is_empty());
}

#[test]
fn peer_upsert_round_trip() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    peers::upsert_peer(&store, &sample_card(), Utc::now()).unwrap();
    let list = peers::list_peers(&store).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].card.name, "test-peer");
}

#[test]
fn inbound_task_lifecycle() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    let msg = Message { role: Role::User, parts: vec![Part::Text { text: "hi".into() }] };
    tasks_in::insert(&store, "t1", "peer-a", &msg).unwrap();
    let pending = tasks_in::list_pending(&store).unwrap();
    assert_eq!(pending.len(), 1);
    let response = Message { role: Role::Agent, parts: vec![Part::Text { text: "ok".into() }] };
    tasks_in::set_response(&store, "t1", &response, TaskState::Completed).unwrap();
    let got = tasks_in::get(&store, "t1").unwrap().unwrap();
    assert_eq!(got.state, TaskState::Completed);
    assert!(tasks_in::list_pending(&store).unwrap().is_empty());
}

#[test]
fn outbound_task_lifecycle() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    let msg = Message { role: Role::User, parts: vec![Part::Text { text: "go".into() }] };
    tasks_out::insert(&store, "t9", "peer-b", &msg).unwrap();
    let response = Message { role: Role::Agent, parts: vec![Part::Text { text: "done".into() }] };
    tasks_out::set_response(&store, "t9", &response, TaskState::Completed).unwrap();
    let got = tasks_out::get(&store, "t9").unwrap().unwrap();
    assert_eq!(got.state, TaskState::Completed);
}
```

- [ ] **Step 3: Run tests (expect compile failure)**

```bash
cargo test -p constellation-store
```

- [ ] **Step 4: Implement `schema.rs`**

```rust
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
```

- [ ] **Step 5: Implement `lib.rs`**

```rust
//! SQLite persistence for Constellation peers and tasks.

mod schema;
pub mod peers;
pub mod tasks_in;
pub mod tasks_out;

use rusqlite::Connection;
use std::{path::Path, sync::Mutex};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("lock poisoned")]
    LockPoisoned,
}

pub type Result<T> = std::result::Result<T, StoreError>;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(schema::SCHEMA)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub(crate) fn with_conn<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let guard = self.conn.lock().map_err(|_| StoreError::LockPoisoned)?;
        f(&guard)
    }
}
```

- [ ] **Step 6: Implement `peers.rs`**

```rust
use chrono::{DateTime, Utc};
use constellation_a2a::AgentCard;
use rusqlite::params;

use crate::{Result, Store};

#[derive(Debug, Clone)]
pub struct PeerRecord {
    pub id: String,
    pub card: AgentCard,
    pub last_seen: DateTime<Utc>,
}

pub fn upsert_peer(store: &Store, card: &AgentCard, last_seen: DateTime<Utc>) -> Result<()> {
    let id = card.url.as_str().to_string();
    let card_json = serde_json::to_string(card)?;
    let url = card.url.as_str().to_string();
    let last = last_seen.to_rfc3339();
    store.with_conn(|conn| {
        conn.execute(
            "INSERT INTO peers(id, name, url, card_json, last_seen) VALUES (?1,?2,?3,?4,?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, url=excluded.url,
                 card_json=excluded.card_json, last_seen=excluded.last_seen",
            params![id, card.name, url, card_json, last],
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
                .map_err(|e| crate::StoreError::Json(serde_json::Error::io(
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))))?
                .with_timezone(&Utc);
            out.push(PeerRecord { id, card, last_seen });
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
```

- [ ] **Step 7: Implement `tasks_in.rs`**

```rust
use chrono::Utc;
use constellation_a2a::{Message, TaskState};
use rusqlite::params;

use crate::{Result, Store};

#[derive(Debug, Clone)]
pub struct InTask {
    pub task_id: String,
    pub from_peer: String,
    pub state: TaskState,
    pub request: Message,
    pub response: Option<Message>,
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

pub fn set_response(store: &Store, task_id: &str, response: &Message, state: TaskState) -> Result<()> {
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
            "SELECT task_id, from_peer, state, request_json, response_json FROM tasks_in WHERE task_id=?1",
        )?;
        let mut rows = stmt.query(params![task_id])?;
        if let Some(row) = rows.next()? {
            let task_id: String = row.get(0)?;
            let from_peer: String = row.get(1)?;
            let state: String = row.get(2)?;
            let request_json: String = row.get(3)?;
            let response_json: Option<String> = row.get(4)?;
            let request: Message = serde_json::from_str(&request_json)?;
            let response = response_json
                .as_deref()
                .map(|s| serde_json::from_str::<Message>(s))
                .transpose()?;
            let state = parse_state(&state);
            Ok(Some(InTask { task_id, from_peer, state, request, response }))
        } else {
            Ok(None)
        }
    })
}

pub fn list_pending(store: &Store) -> Result<Vec<InTask>> {
    list_with_states(store, &["submitted", "working", "input-required"])
}

pub fn list_all(store: &Store) -> Result<Vec<InTask>> {
    list_with_states(store, &["submitted", "working", "input-required", "completed", "canceled", "failed"])
}

fn list_with_states(store: &Store, states: &[&str]) -> Result<Vec<InTask>> {
    store.with_conn(|conn| {
        let placeholders = states.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT task_id, from_peer, state, request_json, response_json
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
                Ok((task_id, from_peer, state, request_json, response_json))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (task_id, from_peer, state, request_json, response_json) in rows {
            let request: Message = serde_json::from_str(&request_json)?;
            let response = response_json
                .as_deref()
                .map(|s| serde_json::from_str::<Message>(s))
                .transpose()?;
            out.push(InTask {
                task_id,
                from_peer,
                state: parse_state(&state),
                request,
                response,
            });
        }
        Ok(out)
    })
}

fn parse_state(s: &str) -> TaskState {
    match s {
        "submitted" => TaskState::Submitted,
        "working" => TaskState::Working,
        "input-required" => TaskState::InputRequired,
        "completed" => TaskState::Completed,
        "canceled" => TaskState::Canceled,
        "failed" => TaskState::Failed,
        _ => TaskState::Unknown,
    }
}
```

- [ ] **Step 8: Implement `tasks_out.rs`**

```rust
use chrono::Utc;
use constellation_a2a::{Message, TaskState};
use rusqlite::params;

use crate::{Result, Store};

#[derive(Debug, Clone)]
pub struct OutTask {
    pub task_id: String,
    pub to_peer: String,
    pub state: TaskState,
    pub request: Message,
    pub response: Option<Message>,
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

pub fn set_response(store: &Store, task_id: &str, response: &Message, state: TaskState) -> Result<()> {
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
            "SELECT task_id, to_peer, state, request_json, response_json FROM tasks_out WHERE task_id=?1",
        )?;
        let mut rows = stmt.query(params![task_id])?;
        if let Some(row) = rows.next()? {
            let task_id: String = row.get(0)?;
            let to_peer: String = row.get(1)?;
            let state: String = row.get(2)?;
            let request_json: String = row.get(3)?;
            let response_json: Option<String> = row.get(4)?;
            let request: Message = serde_json::from_str(&request_json)?;
            let response = response_json
                .as_deref()
                .map(|s| serde_json::from_str::<Message>(s))
                .transpose()?;
            let state = match state.as_str() {
                "submitted" => TaskState::Submitted,
                "working" => TaskState::Working,
                "input-required" => TaskState::InputRequired,
                "completed" => TaskState::Completed,
                "canceled" => TaskState::Canceled,
                "failed" => TaskState::Failed,
                _ => TaskState::Unknown,
            };
            Ok(Some(OutTask { task_id, to_peer, state, request, response }))
        } else {
            Ok(None)
        }
    })
}
```

- [ ] **Step 9: Run tests and verify they pass**

```bash
cargo test -p constellation-store
```

Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add crates/constellation-store
git commit -m "feat(store): add sqlite-backed peer and task store"
```

---

## Task 4: Setup prompt template, SECURITY.md

**Files:**
- Create: `docs/setup-prompt.md`
- Create: `docs/SECURITY.md`

- [ ] **Step 1: Write `docs/setup-prompt.md`**

```markdown
# Constellation A2A — Setup Prompt

You are an AI coding agent that has been added to a Constellation A2A mesh. The mesh
is a private network of peers, each running a `constellation` binary on their device,
discovering each other automatically over Tailscale or the local network, and
communicating via the Agent2Agent (A2A) JSON-RPC protocol.

## Your identity

- **Agent name:** `{{AGENT_NAME}}`
- **Skills:** `{{AGENT_SKILLS}}`
- **Local A2A endpoint:** `{{LOCAL_URL}}`
- **Store:** `{{STORE_PATH}}`

Other peers on the mesh see you under this name with these skills. Be honest about
what you can and cannot do.

## Tools you may call

You drive the mesh through the `constellation` shell command. Available verbs:

| Command                            | Purpose                                                                 |
| ---------------------------------- | ----------------------------------------------------------------------- |
| `constellation peers`              | List currently-known peers (name, url, skills).                         |
| `constellation send <peer> <text>` | Send an A2A task to `<peer>`. Prints the task id.                       |
| `constellation wait <task-id>`     | Wait for a previously-sent task to reach `completed` or `failed`.        |
| `constellation inbox`              | List inbound tasks awaiting a response.                                 |
| `constellation respond <id> <text>`| Mark inbound task `<id>` complete with `<text>` as the answer.          |
| `constellation card`               | Print your own agent card (for debugging).                              |

`constellation serve` is already running as a background service and should not
be started manually.

## Operating contract

1. **Inbox loop.** Whenever the user is idle or you finish a step, call
   `constellation inbox`. Inbox lines look like
   `task-abc  peer-name  one-line preview`.

2. **Inbound task handling.** For each inbound task:
   - If it lies within `{{AGENT_SKILLS}}`, do the work locally and call
     `constellation respond <task-id> <answer>`.
   - If it does not, decline politely:
     `constellation respond <task-id> "out of scope: this peer's skills are X"`.
   - Never silently ignore an inbound task.

3. **Outbound delegation.** When the user asks for something outside your skills:
   - Run `constellation peers` to see who is reachable.
   - Pick the peer whose skills best match the request.
   - `constellation send <peer> "<task description>"` — capture the task id.
   - `constellation wait <task-id>` — block until the answer arrives.
   - Relay the answer to the user.

4. **No fabrication.** If no peer can do the work and you cannot, say so and
   stop.

## Worked example — outbound

User: "Find the latest Conduit Matrix release."

You: `constellation peers` → returns `kraken: research, web`.
You: `constellation send kraken "Find the latest Conduit Matrix release."` → task `t-0001`.
You: `constellation wait t-0001` → returns "Conduit 0.10.5 was released on …".
You (to user): "Conduit 0.10.5 was released on …".

## Worked example — inbound

`constellation inbox` returns:
```
t-0042  atmos-vnic  Translate the README of constellation to French
```

You: translate it locally.
You: `constellation respond t-0042 "<translated README>"`.

## Failure modes

- `constellation peers` returns empty: discovery has not yet found peers. Wait 30s,
  retry. If still empty, surface the issue to the user.
- `constellation wait` times out (60s default): the peer is offline or is
  taking long. Retry once. If it fails again, return the error to the user.
- A peer responds with `failed`: include the failure text in your reply to the
  user.

## Out of scope

- Do not invent A2A subcommands. Only use the verbs above.
- Do not bypass the CLI to send raw HTTP — that is the binary's job.
- Do not send cards or tasks to peers you have not discovered.
```

- [ ] **Step 2: Write `docs/SECURITY.md`**

```markdown
# Security model

Constellation A2A trusts its transport. The binary itself does not
authenticate callers and does not encrypt traffic — the assumption is that
peers reach each other over a [Tailscale](https://tailscale.com/) tailnet,
which provides device identity and WireGuard encryption between peers.

## Trust boundary

- **Trusted:** every device on your tailnet (or the LAN, if `mdns` discovery
  is enabled and `bind` is set to a LAN-reachable address).
- **Untrusted:** the public internet. The binary must never be exposed
  there.

## Default bind

`bind = "0.0.0.0:7777"` so the listener is reachable on whichever interface
you advertise. **If you are not on a tailnet and your LAN is hostile**, change
`bind` to one of:

- `127.0.0.1:7777` — local only (useful for dev / single-host testing).
- A specific Tailscale IP such as `100.76.147.110:7777` — only the tailnet.

## Hardening checklist

1. Run `tailscale status` and confirm the device is in the expected tailnet.
2. Set `[network] discovery = ["tailscale"]` (drop `mdns`) on hosts that
   should never accept LAN peers.
3. Set `bind` to your Tailscale IP, not `0.0.0.0`, on multi-homed boxes.
4. Run `./scripts/security-check.sh` before exposing a new node — it asserts
   the bind is not a public IP and runs `cargo audit`.
5. Rotate Tailscale auth keys via the Tailscale admin console if a peer is
   suspected compromised.

## What the binary does **not** do

- It does not execute task content. The LLM agent reads tasks and decides what
  to do; the binary only persists and forwards.
- It does not store secrets in agent cards.
- It does not expose any verbs beyond the documented A2A JSON-RPC methods and
  `/.well-known/agent.json`.

## Reporting

Report vulnerabilities privately to `thekozugroup@gmail.com`.
```

- [ ] **Step 3: Commit**

```bash
git add docs/setup-prompt.md docs/SECURITY.md
git commit -m "docs: add LLM setup prompt and security model"
```

---

## Task 5: `constellation-discovery` — tailscale + mdns

**Files:**
- Create: `crates/constellation-discovery/Cargo.toml`
- Create: `crates/constellation-discovery/src/lib.rs`
- Create: `crates/constellation-discovery/src/probe.rs`
- Create: `crates/constellation-discovery/src/tailscale.rs`
- Create: `crates/constellation-discovery/src/mdns.rs`
- Create: `crates/constellation-discovery/tests/probe_tests.rs`
- Create: `crates/constellation-discovery/tests/tailscale_parse_tests.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "constellation-discovery"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
anyhow = { workspace = true }
constellation-a2a = { path = "../constellation-a2a" }
mdns-sd = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
url = { workspace = true }

[dev-dependencies]
wiremock = { workspace = true }
tokio = { workspace = true, features = ["test-util", "macros"] }
```

- [ ] **Step 2: Write the failing tests**

`crates/constellation-discovery/tests/probe_tests.rs`:

```rust
use constellation_a2a::{AgentCapabilities, AgentCard, Skill};
use constellation_discovery::probe::probe_card;
use url::Url;
use wiremock::{matchers::{method, path}, Mock, MockServer, ResponseTemplate};

fn sample_card(url: &str) -> AgentCard {
    AgentCard {
        name: "probed".into(),
        description: None,
        url: Url::parse(url).unwrap(),
        version: "0.1.0".into(),
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec!["text".into()],
        default_output_modes: vec!["text".into()],
        skills: vec![Skill { id: "x".into(), name: "x".into(), description: None, tags: vec![] }],
    }
}

#[tokio::test]
async fn probe_returns_card_on_200() {
    let server = MockServer::start().await;
    let card = sample_card(&server.uri());
    Mock::given(method("GET"))
        .and(path("/.well-known/agent.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&card))
        .mount(&server)
        .await;
    let got = probe_card(&server.uri()).await.expect("probe ok");
    assert_eq!(got.name, "probed");
}

#[tokio::test]
async fn probe_returns_none_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/agent.json"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let res = probe_card(&server.uri()).await;
    assert!(res.is_err());
}
```

`crates/constellation-discovery/tests/tailscale_parse_tests.rs`:

```rust
use constellation_discovery::tailscale::parse_status_json;

const FIXTURE: &str = r#"{
  "Self": { "TailscaleIPs": ["100.76.147.110"], "Online": true, "HostName": "atmos-vnic" },
  "Peer": {
    "abc": { "TailscaleIPs": ["100.76.147.42"], "Online": true, "HostName": "kraken" },
    "def": { "TailscaleIPs": ["100.76.147.43"], "Online": false, "HostName": "offline" }
  }
}"#;

#[test]
fn parses_online_peers_only() {
    let peers = parse_status_json(FIXTURE).expect("parse");
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].host, "kraken");
    assert_eq!(peers[0].ip.to_string(), "100.76.147.42");
}
```

- [ ] **Step 3: Run tests (expect compile failure)**

```bash
cargo test -p constellation-discovery
```

- [ ] **Step 4: Implement `lib.rs`**

```rust
//! Peer discovery — Tailscale and mDNS implementations.

pub mod mdns;
pub mod probe;
pub mod tailscale;

use constellation_a2a::AgentCard;
use std::net::IpAddr;

#[derive(Debug, Clone)]
pub struct DiscoveredPeer {
    pub host: String,
    pub ip: IpAddr,
    pub port: u16,
    pub card: AgentCard,
}

#[async_trait::async_trait]
pub trait Discoverer: Send + Sync {
    async fn poll(&self) -> Vec<DiscoveredPeer>;
    fn name(&self) -> &'static str;
}
```

- [ ] **Step 5: Update `Cargo.toml` to add `async-trait`**

Append to `[dependencies]`:

```toml
async-trait = "0.1"
```

- [ ] **Step 6: Implement `probe.rs`**

```rust
use anyhow::{anyhow, Result};
use constellation_a2a::AgentCard;
use std::time::Duration;

pub async fn probe_card(base_url: &str) -> Result<AgentCard> {
    let url = format!("{}/.well-known/agent.json", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()?;
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("probe returned status {}", resp.status()));
    }
    let card: AgentCard = resp.json().await?;
    Ok(card)
}
```

- [ ] **Step 7: Implement `tailscale.rs`**

```rust
use anyhow::{Context, Result};
use serde::Deserialize;
use std::net::IpAddr;
use std::time::Duration;

use crate::{probe::probe_card, DiscoveredPeer, Discoverer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailscalePeer {
    pub host: String,
    pub ip: IpAddr,
}

#[derive(Deserialize)]
struct StatusJson {
    #[serde(rename = "Self")]
    self_: Option<NodeJson>,
    #[serde(rename = "Peer", default)]
    peer: std::collections::HashMap<String, NodeJson>,
}

#[derive(Deserialize)]
struct NodeJson {
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Vec<String>,
    #[serde(rename = "Online", default)]
    online: bool,
    #[serde(rename = "HostName", default)]
    host_name: String,
}

pub fn parse_status_json(raw: &str) -> Result<Vec<TailscalePeer>> {
    let parsed: StatusJson = serde_json::from_str(raw).context("parse tailscale status")?;
    let mut out = Vec::new();
    for (_id, node) in parsed.peer {
        if !node.online { continue; }
        if let Some(ip) = node.tailscale_ips.first() {
            if let Ok(parsed) = ip.parse() {
                out.push(TailscalePeer { host: node.host_name, ip: parsed });
            }
        }
    }
    Ok(out)
}

pub async fn fetch_status() -> Result<String> {
    let out = tokio::process::Command::new("tailscale")
        .arg("status")
        .arg("--json")
        .output()
        .await
        .context("invoke `tailscale status --json`")?;
    if !out.status.success() {
        anyhow::bail!("tailscale status exited with status {}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Debug, Clone)]
pub struct TailscaleDiscoverer {
    pub port: u16,
    pub probe_timeout: Duration,
}

impl Default for TailscaleDiscoverer {
    fn default() -> Self {
        Self { port: 7777, probe_timeout: Duration::from_secs(3) }
    }
}

#[async_trait::async_trait]
impl Discoverer for TailscaleDiscoverer {
    fn name(&self) -> &'static str { "tailscale" }

    async fn poll(&self) -> Vec<DiscoveredPeer> {
        let raw = match fetch_status().await {
            Ok(s) => s,
            Err(e) => {
                tracing::debug!(error=%e, "tailscale status unavailable");
                return vec![];
            }
        };
        let peers = match parse_status_json(&raw) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error=%e, "could not parse tailscale status");
                return vec![];
            }
        };
        let port = self.port;
        let mut out = Vec::new();
        for peer in peers {
            let base = format!("http://{}:{port}", peer.ip);
            match probe_card(&base).await {
                Ok(card) => out.push(DiscoveredPeer {
                    host: peer.host,
                    ip: peer.ip,
                    port,
                    card,
                }),
                Err(e) => tracing::debug!(host=%peer.host, error=%e, "probe failed"),
            }
        }
        out
    }
}
```

- [ ] **Step 8: Implement `mdns.rs`**

```rust
use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::time::sleep;

use crate::{probe::probe_card, DiscoveredPeer, Discoverer};

pub const SERVICE_TYPE: &str = "_a2a._tcp.local.";

pub struct MdnsDiscoverer {
    daemon: ServiceDaemon,
    pub local_name: String,
    pub poll_window: Duration,
}

impl MdnsDiscoverer {
    pub fn new(local_name: impl Into<String>) -> anyhow::Result<Self> {
        let daemon = ServiceDaemon::new()?;
        Ok(Self {
            daemon,
            local_name: local_name.into(),
            poll_window: Duration::from_millis(800),
        })
    }

    pub fn advertise(&self, name: &str, ip: IpAddr, port: u16) -> anyhow::Result<()> {
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            name,
            &format!("{name}.local."),
            ip,
            port,
            &[("name", name)][..],
        )?;
        self.daemon.register(info)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Discoverer for MdnsDiscoverer {
    fn name(&self) -> &'static str { "mdns" }

    async fn poll(&self) -> Vec<DiscoveredPeer> {
        let receiver = match self.daemon.browse(SERVICE_TYPE) {
            Ok(rx) => rx,
            Err(e) => {
                tracing::warn!(error=%e, "mdns browse failed");
                return vec![];
            }
        };
        let mut out = Vec::new();
        let deadline = std::time::Instant::now() + self.poll_window;
        loop {
            if std::time::Instant::now() >= deadline { break; }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            tokio::select! {
                _ = sleep(remaining) => break,
                evt = tokio::task::spawn_blocking({
                    let r = receiver.clone();
                    move || r.recv_timeout(Duration::from_millis(200))
                }) => {
                    match evt {
                        Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                            let host_name = info.get_fullname()
                                .trim_end_matches(SERVICE_TYPE)
                                .trim_end_matches('.')
                                .to_string();
                            if host_name == self.local_name { continue; }
                            for ip in info.get_addresses() {
                                let base = format!("http://{}:{}", ip, info.get_port());
                                if let Ok(card) = probe_card(&base).await {
                                    out.push(DiscoveredPeer {
                                        host: host_name.clone(),
                                        ip: *ip,
                                        port: info.get_port(),
                                        card,
                                    });
                                    break;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        out
    }
}
```

- [ ] **Step 9: Run tests and verify they pass**

```bash
cargo test -p constellation-discovery
```

Expected: 3 passed.

- [ ] **Step 10: Commit**

```bash
git add crates/constellation-discovery
git commit -m "feat(discovery): add tailscale + mdns peer discovery"
```

---

## Task 6: `constellation-client` — A2A HTTP client

**Files:**
- Create: `crates/constellation-client/Cargo.toml`
- Create: `crates/constellation-client/src/lib.rs`
- Create: `crates/constellation-client/tests/client_tests.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "constellation-client"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
anyhow = { workspace = true }
constellation-a2a = { path = "../constellation-a2a" }
reqwest = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros"] }
wiremock = { workspace = true }
chrono = { workspace = true }
```

- [ ] **Step 2: Write the failing test**

`crates/constellation-client/tests/client_tests.rs`:

```rust
use chrono::Utc;
use constellation_a2a::{
    JsonRpcResponse, Message, Part, Role, TaskGetResult, TaskState, TaskStatus,
};
use constellation_client::A2aClient;
use serde_json::json;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

fn fake_result(id: &str) -> TaskGetResult {
    TaskGetResult {
        id: id.into(),
        status: TaskStatus { state: TaskState::Completed, timestamp: Utc::now() },
        history: vec![Message {
            role: Role::Agent,
            parts: vec![Part::Text { text: "ok".into() }],
        }],
    }
}

#[tokio::test]
async fn send_task_round_trip() {
    let server = MockServer::start().await;
    let result = fake_result("t-1");
    let response: JsonRpcResponse<TaskGetResult> = JsonRpcResponse::ok(json!("1"), result);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response))
        .mount(&server)
        .await;
    let client = A2aClient::new();
    let msg = Message { role: Role::User, parts: vec![Part::Text { text: "hi".into() }] };
    let task = client.send_task(&server.uri(), "t-1", &msg).await.expect("send");
    assert_eq!(task.id, "t-1");
    assert_eq!(task.status.state, TaskState::Completed);
}
```

- [ ] **Step 3: Run test (expect compile failure)**

```bash
cargo test -p constellation-client
```

- [ ] **Step 4: Implement `lib.rs`**

```rust
//! A2A JSON-RPC client used to send tasks to remote peers.

use anyhow::{anyhow, Result};
use constellation_a2a::{
    JsonRpcRequest, JsonRpcResponse, JsonRpcVersion, Message, TaskGetParams, TaskGetResult,
    TaskSendParams,
};
use reqwest::Client;
use serde_json::json;
use std::time::Duration;

#[derive(Clone)]
pub struct A2aClient {
    http: Client,
}

impl Default for A2aClient {
    fn default() -> Self { Self::new() }
}

impl A2aClient {
    pub fn new() -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client builds");
        Self { http }
    }

    pub async fn send_task(
        &self,
        peer_url: &str,
        task_id: &str,
        message: &Message,
    ) -> Result<TaskGetResult> {
        let req = JsonRpcRequest {
            jsonrpc: JsonRpcVersion::default(),
            id: json!(uuid::Uuid::new_v4().to_string()),
            method: "tasks/send".into(),
            params: serde_json::to_value(TaskSendParams {
                id: task_id.into(),
                message: message.clone(),
            })?,
        };
        let resp: JsonRpcResponse<TaskGetResult> =
            self.http.post(peer_url).json(&req).send().await?.json().await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("peer returned JSON-RPC error: {}", err.message));
        }
        resp.result.ok_or_else(|| anyhow!("peer returned no result"))
    }

    pub async fn get_task(&self, peer_url: &str, task_id: &str) -> Result<TaskGetResult> {
        let req = JsonRpcRequest {
            jsonrpc: JsonRpcVersion::default(),
            id: json!(uuid::Uuid::new_v4().to_string()),
            method: "tasks/get".into(),
            params: serde_json::to_value(TaskGetParams { id: task_id.into() })?,
        };
        let resp: JsonRpcResponse<TaskGetResult> =
            self.http.post(peer_url).json(&req).send().await?.json().await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("peer returned JSON-RPC error: {}", err.message));
        }
        resp.result.ok_or_else(|| anyhow!("peer returned no result"))
    }
}
```

- [ ] **Step 5: Run tests and verify they pass**

```bash
cargo test -p constellation-client
```

- [ ] **Step 6: Commit**

```bash
git add crates/constellation-client
git commit -m "feat(client): add A2A JSON-RPC client"
```

---

## Task 7: `constellation-server` — A2A HTTP server

**Files:**
- Create: `crates/constellation-server/Cargo.toml`
- Create: `crates/constellation-server/src/lib.rs`
- Create: `crates/constellation-server/src/state.rs`
- Create: `crates/constellation-server/src/well_known.rs`
- Create: `crates/constellation-server/src/rpc.rs`
- Create: `crates/constellation-server/tests/rpc_tests.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "constellation-server"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true

[dependencies]
anyhow = { workspace = true }
axum = { workspace = true }
constellation-a2a = { path = "../constellation-a2a" }
constellation-store = { path = "../constellation-store" }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tower = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros"] }
tempfile = { workspace = true }
reqwest = { workspace = true }
url = { workspace = true }
```

- [ ] **Step 2: Write the failing test**

`crates/constellation-server/tests/rpc_tests.rs`:

```rust
use constellation_a2a::{
    AgentCapabilities, AgentCard, JsonRpcResponse, Message, Part, Role, Skill, TaskGetResult,
    TaskState,
};
use constellation_server::{build_app, AppState};
use constellation_store::Store;
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tempfile::tempdir;
use tokio::net::TcpListener;
use url::Url;

fn card() -> AgentCard {
    AgentCard {
        name: "self".into(),
        description: None,
        url: Url::parse("http://127.0.0.1:0").unwrap(),
        version: "0.1.0".into(),
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec!["text".into()],
        default_output_modes: vec!["text".into()],
        skills: vec![Skill { id: "x".into(), name: "x".into(), description: None, tags: vec![] }],
    }
}

#[tokio::test]
async fn tasks_send_persists_inbound() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("s.db")).unwrap());
    let state = AppState { store: store.clone(), card: card() };

    let app = build_app(state);
    let listener = TcpListener::bind(SocketAddr::from(([127,0,0,1], 0))).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let body = json!({
        "jsonrpc":"2.0","id":"1","method":"tasks/send",
        "params":{"id":"t1","message":{"role":"user","parts":[{"type":"text","text":"hi"}]}}
    });
    let resp: JsonRpcResponse<TaskGetResult> = reqwest::Client::new()
        .post(format!("http://{addr}"))
        .json(&body)
        .send().await.unwrap()
        .json().await.unwrap();
    assert!(resp.error.is_none(), "{:?}", resp.error);
    assert_eq!(resp.result.unwrap().status.state, TaskState::Submitted);

    // confirm persisted
    let stored = constellation_store::tasks_in::get(&store, "t1").unwrap().unwrap();
    assert_eq!(stored.from_peer, "unknown");
    let _ = stored.request;
}

#[tokio::test]
async fn agent_card_endpoint_returns_card() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("s.db")).unwrap());
    let state = AppState { store, card: card() };
    let app = build_app(state);
    let listener = TcpListener::bind(SocketAddr::from(([127,0,0,1], 0))).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let resp: AgentCard = reqwest::get(format!("http://{addr}/.well-known/agent.json"))
        .await.unwrap().json().await.unwrap();
    assert_eq!(resp.name, "self");
}
```

- [ ] **Step 3: Run tests (expect compile failure)**

```bash
cargo test -p constellation-server
```

- [ ] **Step 4: Implement `state.rs`**

```rust
use constellation_a2a::AgentCard;
use constellation_store::Store;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub card: AgentCard,
}
```

- [ ] **Step 5: Implement `well_known.rs`**

```rust
use axum::{extract::State, Json};
use constellation_a2a::AgentCard;

use crate::state::AppState;

pub async fn get_agent_card(State(state): State<AppState>) -> Json<AgentCard> {
    Json(state.card.clone())
}
```

- [ ] **Step 6: Implement `rpc.rs`**

```rust
use axum::{extract::State, Json};
use chrono::Utc;
use constellation_a2a::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, Message, TaskGetParams, TaskGetResult,
    TaskSendParams, TaskState, TaskStatus,
};
use constellation_store::{tasks_in, Store};
use std::sync::Arc;

use crate::state::AppState;

pub async fn dispatch(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Json<serde_json::Value> {
    let id = req.id.clone();
    let body = match req.method.as_str() {
        "tasks/send" => handle_send(&state.store, req).await,
        "tasks/get" => handle_get(&state.store, req).await,
        "tasks/cancel" => Err(JsonRpcError::method_not_found("tasks/cancel")),
        other => Err(JsonRpcError::method_not_found(other)),
    };
    let response = match body {
        Ok(value) => JsonRpcResponse::<serde_json::Value>::ok(id, value),
        Err(e) => JsonRpcResponse::<serde_json::Value>::err(id, e),
    };
    Json(serde_json::to_value(response).unwrap())
}

async fn handle_send(store: &Arc<Store>, req: JsonRpcRequest) -> Result<serde_json::Value, JsonRpcError> {
    let params: TaskSendParams = serde_json::from_value(req.params)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    tasks_in::insert(store, &params.id, "unknown", &params.message)
        .map_err(|e| JsonRpcError::internal_error(e.to_string()))?;
    let result = TaskGetResult {
        id: params.id.clone(),
        status: TaskStatus { state: TaskState::Submitted, timestamp: Utc::now() },
        history: vec![params.message],
    };
    Ok(serde_json::to_value(result).unwrap())
}

async fn handle_get(store: &Arc<Store>, req: JsonRpcRequest) -> Result<serde_json::Value, JsonRpcError> {
    let params: TaskGetParams = serde_json::from_value(req.params)
        .map_err(|e| JsonRpcError::invalid_params(e.to_string()))?;
    let task = tasks_in::get(store, &params.id)
        .map_err(|e| JsonRpcError::internal_error(e.to_string()))?
        .ok_or_else(|| JsonRpcError::task_not_found(&params.id))?;
    let mut history: Vec<Message> = vec![task.request];
    if let Some(resp) = task.response { history.push(resp); }
    let result = TaskGetResult {
        id: task.task_id,
        status: TaskStatus { state: task.state, timestamp: Utc::now() },
        history,
    };
    Ok(serde_json::to_value(result).unwrap())
}
```

- [ ] **Step 7: Implement `lib.rs`**

```rust
//! Axum router for the A2A server.

pub mod rpc;
pub mod state;
pub mod well_known;

use axum::{routing::{get, post}, Router};
pub use state::AppState;

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/", post(rpc::dispatch))
        .route("/.well-known/agent.json", get(well_known::get_agent_card))
        .with_state(state)
}
```

- [ ] **Step 8: Run tests and verify they pass**

```bash
cargo test -p constellation-server
```

- [ ] **Step 9: Commit**

```bash
git add crates/constellation-server
git commit -m "feat(server): add A2A HTTP server with tasks/send and tasks/get"
```

---

## Task 8: `constellation-cli` — binary

**Files:**
- Create: `crates/constellation-cli/Cargo.toml`
- Create: `crates/constellation-cli/src/main.rs`
- Create: `crates/constellation-cli/src/config.rs`
- Create: `crates/constellation-cli/src/net.rs`
- Create: `crates/constellation-cli/src/prompt.rs`
- Create: `crates/constellation-cli/src/commands/mod.rs` (and one file per subcommand listed below)
- Create: `crates/constellation-cli/src/commands/{init,serve,peers,send,wait,inbox,respond,card,install_service}.rs`
- Create: `crates/constellation-cli/assets/constellation.service.tmpl`
- Create: `crates/constellation-cli/tests/cli_smoke.rs`

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "constellation-cli"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true

[[bin]]
name = "constellation"
path = "src/main.rs"

[dependencies]
anyhow = { workspace = true }
clap = { workspace = true }
constellation-a2a = { path = "../constellation-a2a" }
constellation-client = { path = "../constellation-client" }
constellation-discovery = { path = "../constellation-discovery" }
constellation-server = { path = "../constellation-server" }
constellation-store = { path = "../constellation-store" }
chrono = { workspace = true }
dirs = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "process", "signal", "fs"] }
toml = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
url = { workspace = true }
uuid = { workspace = true }
async-trait = "0.1"
```

- [ ] **Step 2: Write the smoke test**

`crates/constellation-cli/tests/cli_smoke.rs`:

```rust
use std::process::Command;

#[test]
fn binary_prints_help() {
    let exe = env!("CARGO_BIN_EXE_constellation");
    let output = Command::new(exe).arg("--help").output().expect("run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for verb in ["init","serve","peers","send","wait","inbox","respond","card","install-service"] {
        assert!(stdout.contains(verb), "help missing verb: {verb}");
    }
}
```

- [ ] **Step 3: Implement `config.rs`**

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agent: AgentSection,
    pub network: NetworkSection,
    pub store: StoreSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSection {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSection {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_advertised")]
    pub advertised_host: String,
    #[serde(default = "default_discovery")]
    pub discovery: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSection {
    #[serde(default = "default_store")]
    pub path: String,
}

fn default_bind() -> String { "0.0.0.0:7777".into() }
fn default_advertised() -> String { "auto".into() }
fn default_discovery() -> Vec<String> { vec!["tailscale".into(), "mdns".into()] }
fn default_store() -> String { "auto".into() }

impl Config {
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("constellation/config.toml")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        Ok(toml::from_str(&raw)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    pub fn store_path(&self) -> PathBuf {
        if self.store.path == "auto" {
            dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
                .join("constellation/store.db")
        } else {
            PathBuf::from(&self.store.path)
        }
    }
}
```

- [ ] **Step 4: Implement `net.rs`**

```rust
use anyhow::{anyhow, Result};

pub async fn resolve_advertised_host(setting: &str) -> Result<String> {
    if setting != "auto" {
        return Ok(setting.to_string());
    }
    if let Some(ip) = tailscale_ip().await { return Ok(ip); }
    if let Some(ip) = first_lan_ipv4()? { return Ok(ip); }
    Err(anyhow!("could not resolve a non-loopback IP for advertisement"))
}

async fn tailscale_ip() -> Option<String> {
    let out = tokio::process::Command::new("tailscale")
        .arg("ip").arg("-4").output().await.ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_string())
}

fn first_lan_ipv4() -> Result<Option<String>> {
    use std::net::{IpAddr, Ipv4Addr};
    let interfaces = pnet_simple_iter()?;
    for ip in interfaces {
        if let IpAddr::V4(v4) = ip {
            if v4 != Ipv4Addr::LOCALHOST && !v4.is_loopback() && !v4.is_unspecified() {
                return Ok(Some(v4.to_string()));
            }
        }
    }
    Ok(None)
}

#[cfg(unix)]
fn pnet_simple_iter() -> Result<Vec<std::net::IpAddr>> {
    use std::net::IpAddr;
    let mut out = Vec::new();
    let raw = std::process::Command::new("hostname").arg("-I").output()?;
    if !raw.status.success() { return Ok(out); }
    for tok in String::from_utf8_lossy(&raw.stdout).split_whitespace() {
        if let Ok(addr) = tok.parse::<IpAddr>() { out.push(addr); }
    }
    Ok(out)
}

#[cfg(not(unix))]
fn pnet_simple_iter() -> Result<Vec<std::net::IpAddr>> { Ok(vec![]) }
```

- [ ] **Step 5: Implement `prompt.rs`**

```rust
const TEMPLATE: &str = include_str!("../../../docs/setup-prompt.md");

pub fn render(agent_name: &str, skills: &[String], local_url: &str, store_path: &str) -> String {
    TEMPLATE
        .replace("{{AGENT_NAME}}", agent_name)
        .replace("{{AGENT_SKILLS}}", &skills.join(", "))
        .replace("{{LOCAL_URL}}", local_url)
        .replace("{{STORE_PATH}}", store_path)
}
```

- [ ] **Step 6: Implement `main.rs`**

```rust
mod commands;
mod config;
mod net;
mod prompt;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "constellation",
    about = "P2P A2A mesh runtime — see docs/setup-prompt.md",
    version
)]
struct Cli {
    #[arg(long, env = "CONSTELLATION_CONFIG")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Init {
        #[arg(long)] name: Option<String>,
        #[arg(long, value_delimiter = ',')] skills: Option<Vec<String>>,
        #[arg(long)] port: Option<u16>,
    },
    Serve,
    Peers {
        #[arg(long)] json: bool,
    },
    Send {
        peer: String,
        text: String,
    },
    Wait {
        task_id: String,
        #[arg(long, default_value_t = 60)] timeout: u64,
    },
    Inbox {
        #[arg(long)] json: bool,
    },
    Respond {
        task_id: String,
        text: String,
    },
    Card,
    InstallService,
}

fn config_path(cli: &Cli) -> PathBuf {
    cli.config.clone().unwrap_or_else(config::Config::default_path)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,constellation=info")))
        .init();
    let cli = Cli::parse();
    let path = config_path(&cli);
    match cli.cmd {
        Cmd::Init { name, skills, port } => commands::init::run(&path, name, skills, port).await,
        Cmd::Serve => commands::serve::run(&path).await,
        Cmd::Peers { json } => commands::peers::run(&path, json).await,
        Cmd::Send { peer, text } => commands::send::run(&path, &peer, &text).await,
        Cmd::Wait { task_id, timeout } => commands::wait::run(&path, &task_id, timeout).await,
        Cmd::Inbox { json } => commands::inbox::run(&path, json).await,
        Cmd::Respond { task_id, text } => commands::respond::run(&path, &task_id, &text).await,
        Cmd::Card => commands::card::run(&path).await,
        Cmd::InstallService => commands::install_service::run().await,
    }
}
```

- [ ] **Step 7: Implement `commands/mod.rs`**

```rust
pub mod card;
pub mod inbox;
pub mod init;
pub mod install_service;
pub mod peers;
pub mod respond;
pub mod send;
pub mod serve;
pub mod wait;

use crate::config::Config;
use anyhow::{Context, Result};
use constellation_a2a::{AgentCapabilities, AgentCard, Skill};
use std::path::Path;
use url::Url;

pub fn load_config(path: &Path) -> Result<Config> {
    Config::load(path).with_context(|| format!("could not load config at {}", path.display()))
}

pub async fn build_card_from_config(cfg: &Config) -> Result<AgentCard> {
    let host = crate::net::resolve_advertised_host(&cfg.network.advertised_host).await?;
    let port = cfg.network.bind.split(':').nth(1).unwrap_or("7777").parse::<u16>()?;
    let url = Url::parse(&format!("http://{host}:{port}"))?;
    let skills = cfg.agent.skills.iter()
        .map(|s| Skill { id: s.clone(), name: s.clone(), description: None, tags: vec![s.clone()] })
        .collect();
    Ok(AgentCard {
        name: cfg.agent.name.clone(),
        description: cfg.agent.description.clone(),
        url,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec!["text".into()],
        default_output_modes: vec!["text".into()],
        skills,
    })
}
```

- [ ] **Step 8: Implement `commands/init.rs`**

```rust
use anyhow::{Context, Result};
use std::path::Path;

use crate::config::{AgentSection, Config, NetworkSection, StoreSection};
use crate::prompt::render;

pub async fn run(
    path: &Path,
    name: Option<String>,
    skills: Option<Vec<String>>,
    port: Option<u16>,
) -> Result<()> {
    let name = name.unwrap_or_else(|| {
        std::env::var("HOSTNAME")
            .or_else(|_| hostname_from_uname())
            .unwrap_or_else(|_| "constellation-node".to_string())
    });
    let skills = skills.unwrap_or_else(|| vec!["general".to_string()]);
    let port = port.unwrap_or(7777);
    let cfg = Config {
        agent: AgentSection { name: name.clone(), description: None, skills: skills.clone() },
        network: NetworkSection {
            bind: format!("0.0.0.0:{port}"),
            advertised_host: "auto".into(),
            discovery: vec!["tailscale".into(), "mdns".into()],
        },
        store: StoreSection { path: "auto".into() },
    };
    cfg.save(path).context("save config")?;
    let local_url = format!("http://<advertised host>:{port}");
    println!("config written: {}\n", path.display());
    println!("--- copy the prompt below into your LLM coding agent ---\n");
    println!("{}", render(&name, &skills, &local_url, &cfg.store_path().display().to_string()));
    Ok(())
}

fn hostname_from_uname() -> Result<String, std::io::Error> {
    let out = std::process::Command::new("hostname").output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
```

- [ ] **Step 9: Implement `commands/serve.rs`**

```rust
use anyhow::Result;
use constellation_discovery::{
    mdns::MdnsDiscoverer, tailscale::TailscaleDiscoverer, DiscoveredPeer, Discoverer,
};
use constellation_server::{build_app, AppState};
use constellation_store::{peers as peers_store, Store};
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tokio::net::TcpListener;

use crate::commands::{build_card_from_config, load_config};

pub async fn run(path: &Path) -> Result<()> {
    let cfg = load_config(path)?;
    let card = build_card_from_config(&cfg).await?;
    let store = Arc::new(Store::open(cfg.store_path())?);
    let bind: SocketAddr = cfg.network.bind.parse()?;
    let listener = TcpListener::bind(bind).await?;
    tracing::info!(%bind, "constellation a2a listener up");

    let app_state = AppState { store: store.clone(), card: card.clone() };
    let app = build_app(app_state);
    let serve_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error=?e, "server exited");
        }
    });

    let port = bind.port();
    let mut discoverers: Vec<Box<dyn Discoverer>> = Vec::new();
    for d in &cfg.network.discovery {
        match d.as_str() {
            "tailscale" => discoverers.push(Box::new(TailscaleDiscoverer { port, ..Default::default() })),
            "mdns" => match MdnsDiscoverer::new(card.name.clone()) {
                Ok(m) => {
                    if let Ok(host_str) = card.url.host_str().ok_or("no host").map(String::from) {
                        if let Ok(ip) = host_str.parse() {
                            let _ = m.advertise(&card.name, ip, port);
                        }
                    }
                    discoverers.push(Box::new(m));
                }
                Err(e) => tracing::warn!(error=?e, "mdns disabled"),
            },
            other => tracing::warn!(%other, "unknown discoverer"),
        }
    }

    let discovery_handle = tokio::spawn(async move {
        loop {
            let mut all: Vec<DiscoveredPeer> = Vec::new();
            for d in &discoverers {
                let mut got = d.poll().await;
                tracing::debug!(target=d.name(), found=got.len(), "discovered");
                all.append(&mut got);
            }
            for peer in all {
                let _ = peers_store::upsert_peer(&store, &peer.card, chrono::Utc::now());
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    tokio::select! {
        _ = serve_handle => {},
        _ = discovery_handle => {},
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown requested");
        }
    }
    Ok(())
}
```

- [ ] **Step 10: Implement `commands/peers.rs`**

```rust
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
            let skills = p.card.skills.iter().map(|s| s.id.as_str()).collect::<Vec<_>>().join(",");
            println!("{}\t{}\t{}", p.card.name, p.card.url, skills);
        }
    }
    Ok(())
}
```

- [ ] **Step 11: Implement `commands/send.rs`**

```rust
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
    let peer = peers.iter().find(|p| p.card.name == peer_name)
        .ok_or_else(|| anyhow!("peer '{peer_name}' not in store. run `constellation peers` first."))?;
    let task_id = format!("t-{}", Uuid::new_v4());
    let msg = Message { role: Role::User, parts: vec![Part::Text { text: text.to_string() }] };
    tasks_out::insert(&store, &task_id, &peer.card.name, &msg)?;
    let client = A2aClient::new();
    let _ = client.send_task(peer.card.url.as_str(), &task_id, &msg).await?;
    println!("{task_id}");
    Ok(())
}
```

- [ ] **Step 12: Implement `commands/wait.rs`**

```rust
use anyhow::{anyhow, Result};
use constellation_a2a::TaskState;
use constellation_client::A2aClient;
use constellation_store::{peers as peers_store, tasks_out, Store};
use std::{path::Path, time::Duration};

use crate::commands::load_config;

pub async fn run(path: &Path, task_id: &str, timeout_secs: u64) -> Result<()> {
    let cfg = load_config(path)?;
    let store = Store::open(cfg.store_path())?;
    let task = tasks_out::get(&store, task_id)?
        .ok_or_else(|| anyhow!("no outbound task with id {task_id}"))?;
    let peers = peers_store::list_peers(&store)?;
    let peer = peers.iter().find(|p| p.card.name == task.to_peer)
        .ok_or_else(|| anyhow!("peer '{}' not in store", task.to_peer))?;
    let client = A2aClient::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let result = client.get_task(peer.card.url.as_str(), task_id).await?;
        match result.status.state {
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                if let Some(last) = result.history.last() {
                    let body = last.parts.iter()
                        .filter_map(|p| match p { constellation_a2a::Part::Text { text } => Some(text.as_str()) })
                        .collect::<Vec<_>>().join("\n");
                    println!("{body}");
                }
                tasks_out::set_state(&store, task_id, result.status.state)?;
                return Ok(());
            }
            _ => {
                if std::time::Instant::now() >= deadline {
                    return Err(anyhow!("timed out waiting for task {task_id}"));
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
```

- [ ] **Step 13: Implement `commands/inbox.rs`**

```rust
use anyhow::Result;
use constellation_store::{tasks_in, Store};
use std::path::Path;

use crate::commands::load_config;

pub async fn run(path: &Path, json: bool) -> Result<()> {
    let cfg = load_config(path)?;
    let store = Store::open(cfg.store_path())?;
    let pending = tasks_in::list_pending(&store)?;
    if json {
        let view: Vec<_> = pending.iter().map(|t| serde_json::json!({
            "task_id": t.task_id,
            "from_peer": t.from_peer,
            "request": t.request,
        })).collect();
        println!("{}", serde_json::to_string_pretty(&view)?);
    } else {
        for t in &pending {
            let preview = t.request.parts.iter()
                .filter_map(|p| match p { constellation_a2a::Part::Text { text } => Some(text.as_str()) })
                .collect::<Vec<_>>().join(" ");
            let preview = preview.chars().take(80).collect::<String>();
            println!("{}\t{}\t{}", t.task_id, t.from_peer, preview);
        }
    }
    Ok(())
}
```

- [ ] **Step 14: Implement `commands/respond.rs`**

```rust
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
    let msg = Message { role: Role::Agent, parts: vec![Part::Text { text: text.to_string() }] };
    tasks_in::set_response(&store, task_id, &msg, TaskState::Completed)?;
    println!("ok");
    Ok(())
}
```

- [ ] **Step 15: Implement `commands/card.rs`**

```rust
use anyhow::Result;
use std::path::Path;

use crate::commands::{build_card_from_config, load_config};

pub async fn run(path: &Path) -> Result<()> {
    let cfg = load_config(path)?;
    let card = build_card_from_config(&cfg).await?;
    println!("{}", serde_json::to_string_pretty(&card)?);
    Ok(())
}
```

- [ ] **Step 16: Implement `commands/install_service.rs`**

```rust
use anyhow::Result;
use std::path::PathBuf;

const TEMPLATE: &str = include_str!("../../assets/constellation.service.tmpl");

pub async fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let unit = TEMPLATE
        .replace("{{EXE}}", &exe.display().to_string())
        .replace("{{USER}}", &whoami_user());
    let target = systemd_user_unit_path()?;
    if let Some(parent) = target.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(&target, unit)?;
    println!("installed: {}", target.display());
    println!("enable with: systemctl --user enable --now constellation");
    Ok(())
}

fn whoami_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "user".into())
}

fn systemd_user_unit_path() -> Result<PathBuf> {
    let base = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    Ok(base.join("systemd/user/constellation.service"))
}
```

- [ ] **Step 17: Write `assets/constellation.service.tmpl`**

```ini
[Unit]
Description=Constellation A2A peer
After=network-online.target
Wants=network-online.target

[Service]
ExecStart={{EXE}} serve
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info,constellation=info

[Install]
WantedBy=default.target
```

- [ ] **Step 18: Build the workspace and run the smoke test**

```bash
cargo build --workspace
cargo test -p constellation-cli
```

Expected: build succeeds, smoke test passes.

- [ ] **Step 19: Commit**

```bash
git add crates/constellation-cli
git commit -m "feat(cli): add constellation binary with serve and CLI verbs"
```

---

## Task 9: End-to-end integration test

**Files:**
- Create: `tests/two_peer_loopback.rs` (workspace-level integration test crate? — instead, create as a test inside `constellation-cli`)
- Create: `crates/constellation-cli/tests/two_peer_loopback.rs`

- [ ] **Step 1: Write the test**

`crates/constellation-cli/tests/two_peer_loopback.rs`:

```rust
//! Spawn two `constellation` processes on loopback. A sends a task to B; B's
//! shell answers it via `constellation respond`; A's `wait` returns the answer.

use std::{
    fs, path::PathBuf, process::{Child, Command, Stdio}, time::Duration,
};
use tempfile::tempdir;

fn write_config(dir: &std::path::Path, name: &str, port: u16) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(&path, format!(r#"
[agent]
name = "{name}"
skills = ["test"]

[network]
bind = "127.0.0.1:{port}"
advertised_host = "127.0.0.1"
discovery = []

[store]
path = "{}"
"#, dir.join("store.db").display())).unwrap();
    path
}

fn spawn_serve(exe: &str, config: &PathBuf) -> Child {
    Command::new(exe)
        .arg("--config").arg(config)
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn().expect("spawn serve")
}

fn cli(exe: &str, config: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(exe).arg("--config").arg(config).args(args)
        .output().expect("cli output")
}

#[test]
fn two_peer_round_trip() {
    let exe = env!("CARGO_BIN_EXE_constellation");
    let a_dir = tempdir().unwrap();
    let b_dir = tempdir().unwrap();
    let a_cfg = write_config(a_dir.path(), "alice", 47771);
    let b_cfg = write_config(b_dir.path(), "bob", 47772);

    let mut a = spawn_serve(exe, &a_cfg);
    let mut b = spawn_serve(exe, &b_cfg);
    std::thread::sleep(Duration::from_millis(800));

    // Manually upsert each side's peer (no Tailscale or mDNS in CI).
    let helper_dir = tempdir().unwrap();
    let helper_cfg = write_config(helper_dir.path(), "helper", 47773);
    drop(helper_cfg);
    // Use SQL via store to register peers symmetrically.
    let alice_url = format!("http://127.0.0.1:47771");
    let bob_url   = format!("http://127.0.0.1:47772");
    insert_peer(a_dir.path(), "bob", &bob_url);
    insert_peer(b_dir.path(), "alice", &alice_url);

    // alice sends to bob
    let send_out = cli(exe, &a_cfg, &["send", "bob", "say hi"]);
    assert!(send_out.status.success(), "send failed: {:?}", send_out);
    let task_id = String::from_utf8_lossy(&send_out.stdout).trim().to_string();

    // bob's inbox shows it
    let inbox = cli(exe, &b_cfg, &["inbox"]);
    assert!(inbox.status.success());
    let inbox = String::from_utf8_lossy(&inbox.stdout);
    assert!(inbox.contains(&task_id), "bob inbox missing task {task_id}: {inbox}");

    // bob responds
    let respond = cli(exe, &b_cfg, &["respond", &task_id, "hi alice"]);
    assert!(respond.status.success());

    // alice waits, sees bob's reply
    let wait = cli(exe, &a_cfg, &["wait", &task_id, "--timeout", "10"]);
    assert!(wait.status.success(), "wait failed: stderr={}",
        String::from_utf8_lossy(&wait.stderr));
    let body = String::from_utf8_lossy(&wait.stdout);
    assert!(body.contains("hi alice"), "expected reply; got: {body}");

    let _ = a.kill();
    let _ = b.kill();
}

fn insert_peer(dir: &std::path::Path, name: &str, url: &str) {
    use rusqlite::Connection;
    let db = dir.join("store.db");
    let conn = Connection::open(&db).expect("open db");
    let card = serde_json::json!({
        "name": name,
        "url": url,
        "version": "0.1.0",
        "capabilities": {"streaming":false,"pushNotifications":false,"stateTransitionHistory":false},
        "defaultInputModes":["text"],
        "defaultOutputModes":["text"],
        "skills":[{"id":"test","name":"test","tags":["test"]}]
    });
    let card_json = card.to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO peers(id,name,url,card_json,last_seen) VALUES (?1,?2,?3,?4,?5)",
        rusqlite::params![url, name, url, card_json, now],
    ).expect("upsert peer");
}
```

- [ ] **Step 2: Add `rusqlite` and `chrono` to dev-dependencies**

Append to `crates/constellation-cli/Cargo.toml`:

```toml
[dev-dependencies]
rusqlite = { workspace = true }
chrono = { workspace = true }
serde_json = { workspace = true }
tempfile = { workspace = true }
```

- [ ] **Step 3: Run the test**

```bash
cargo test -p constellation-cli --test two_peer_loopback
```

Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add crates/constellation-cli
git commit -m "test(cli): add two-peer loopback integration test"
```

---

## Task 10: CI and security scripts

**Files:**
- Modify/Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/audit.yml`
- Create: `scripts/health-check.sh` (rewrite)
- Create: `scripts/security-check.sh` (rewrite)

- [ ] **Step 1: Write `.github/workflows/ci.yml`**

```yaml
name: ci
on:
  push: { branches: [main] }
  pull_request: { branches: [main] }

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - uses: Swatinem/rust-cache@v2
      - name: fmt
        run: cargo fmt --all -- --check
      - name: clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: test
        run: cargo test --workspace --all-targets
```

- [ ] **Step 2: Write `.github/workflows/audit.yml`**

```yaml
name: audit
on:
  schedule: [{ cron: "0 7 * * 1" }]
  workflow_dispatch: {}
jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-audit --locked
      - run: cargo audit
```

- [ ] **Step 3: Rewrite `scripts/health-check.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail
PORT="${PORT:-7777}"
HOST="${HOST:-127.0.0.1}"
URL="http://$HOST:$PORT/.well-known/agent.json"
echo "GET $URL"
curl --fail --silent --show-error --max-time 5 "$URL" | head -c 400
echo
```

- [ ] **Step 4: Rewrite `scripts/security-check.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail
echo "==> cargo audit"
if ! command -v cargo-audit >/dev/null 2>&1; then
  cargo install cargo-audit --locked
fi
cargo audit

echo "==> bind sanity"
CFG="${1:-${HOME}/.config/constellation/config.toml}"
if [ -f "$CFG" ]; then
  bind=$(awk -F\" '/^bind/ { print $2 }' "$CFG")
  if echo "$bind" | grep -Eq '^(0\.0\.0\.0|::):'; then
    echo "WARN: bind=$bind exposes the listener on all interfaces."
    echo "      Acceptable on a tailscale-only host. Otherwise change to a"
    echo "      tailscale or loopback IP. See docs/SECURITY.md."
  else
    echo "OK: bind=$bind"
  fi
fi
```

- [ ] **Step 5: chmod +x scripts**

```bash
chmod +x scripts/health-check.sh scripts/security-check.sh
```

- [ ] **Step 6: Commit**

```bash
git add .github/workflows scripts
git commit -m "ci: rewrite for Rust workspace + add audit workflow"
```

---

## Task 11: Rewrite README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace README**

Write a README that covers, in order:
1. One-line tagline.
2. What this is (P2P A2A mesh, no central server).
3. Architecture (the same diagram as in the spec).
4. Quickstart: `cargo install --path crates/constellation-cli`, `constellation init`, paste the prompt.
5. CLI verbs table (copy from spec).
6. Configuration table.
7. Discovery (`tailscale` first, `mdns` fallback) with the security note.
8. Pointer to `docs/SECURITY.md`, `docs/setup-prompt.md`, and the new spec.

Concrete content:

```markdown
# Constellation A2A

**Peer-to-peer Agent2Agent (A2A) mesh over Tailscale.**

Constellation turns any LLM coding agent (Claude Code, Cursor, Codex, …) into
a peer on a private agent mesh. There is no central server. Each device runs
a single Rust binary that speaks the [A2A protocol](https://a2a-protocol.org/)
and discovers other peers automatically over Tailscale (primary) or mDNS on
the local network (fallback).

```
                ┌───── tailnet (or LAN) ─────┐
   Device A   ◄──────── A2A JSON-RPC ─────────►   Device B
   constellation                                  constellation
   + your LLM agent                               + your LLM agent
```

## Quickstart

```bash
# 1) Install
cargo install --path crates/constellation-cli

# 2) Configure and copy the prompt that gets pasted into your LLM agent
constellation init --name my-box --skills bash,research

# 3) Run as a service
constellation install-service
systemctl --user enable --now constellation

# 4) Verify
constellation card | jq .name
constellation peers
```

## Verbs

| Command                            | Purpose                                                            |
| ---------------------------------- | ------------------------------------------------------------------ |
| `constellation init`               | Write `config.toml` and print the LLM setup prompt.                |
| `constellation serve`              | Run the A2A HTTP server and discovery loop.                        |
| `constellation peers`              | List currently-known peers.                                        |
| `constellation send <peer> <text>` | Send a task to `<peer>`. Prints the task id.                       |
| `constellation wait <task-id>`     | Block until a sent task completes; print the response.             |
| `constellation inbox`              | Print inbound tasks awaiting a response.                           |
| `constellation respond <id> <text>`| Mark inbound task `<id>` complete with `<text>`.                   |
| `constellation card`               | Print this device's agent card.                                    |
| `constellation install-service`    | Install a systemd user unit that runs `constellation serve`.       |

## Configuration

`$XDG_CONFIG_HOME/constellation/config.toml`:

```toml
[agent]
name = "my-box"
description = "Cloud ARM dev box"
skills = ["bash", "research"]

[network]
bind = "0.0.0.0:7777"
advertised_host = "auto"          # tailscale ip > LAN ip
discovery = ["tailscale", "mdns"] # drop "mdns" on hostile LANs

[store]
path = "auto"                     # $XDG_DATA_HOME/constellation/store.db
```

## Security

Constellation trusts its transport. Run it on a Tailscale tailnet and limit
`discovery` accordingly. See [`docs/SECURITY.md`](docs/SECURITY.md).

## License

MIT — Copyright 2026 Tachyon Labs HQ.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: rewrite README for the Rust A2A model"
```

---

## Self-review

Spec coverage check:

- ✅ A2A wire protocol — Task 2.
- ✅ Tailscale + mDNS discovery — Task 5.
- ✅ JSON-RPC server with `/.well-known/agent.json` and `tasks/send` / `tasks/get` — Task 7.
- ✅ Outbound A2A client — Task 6.
- ✅ Sqlite-backed peer / task store — Task 3.
- ✅ CLI binary with `init`, `serve`, `peers`, `send`, `wait`, `inbox`, `respond`, `card`, `install-service` — Task 8.
- ✅ Setup prompt template embedded into binary — Task 4 + 8 step 5.
- ✅ Strip Conduit / Matrix SDK / PyO3 / bundled agents / Docker — Task 1.
- ✅ End-to-end integration test — Task 9.
- ✅ CI rewrite + security script — Task 10.
- ✅ README rewrite — Task 11.

Placeholder scan: no "TBD", no "implement later", no "similar to Task N", no
unspecified types. Type names are consistent across crates: `AgentCard`,
`AgentCapabilities`, `Skill`, `Message`, `Part`, `Role`, `TaskState`,
`TaskStatus`, `TaskGetResult`, `TaskSendParams`, `TaskGetParams`,
`JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`. CLI verbs are spelled
identically in the prompt template, the README, the `clap` derive in
`main.rs`, and the integration test. Discovery types `DiscoveredPeer`,
`Discoverer`, `TailscaleDiscoverer`, `MdnsDiscoverer` are referenced
consistently.
