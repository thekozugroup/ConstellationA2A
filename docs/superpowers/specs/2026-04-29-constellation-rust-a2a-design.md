# Constellation A2A — P2P Rust Agent Mesh over Tailscale

**Date:** 2026-04-29
**Status:** Design
**Supersedes:** `docs/specs/2026-03-26-constellation-a2a-design.md`

## Summary

Rewrite Constellation A2A as a single Rust binary that turns any LLM coding agent
(Claude Code, Cursor, Codex, etc.) into a peer on a private Agent2Agent (A2A)
mesh. No central server, no Matrix, no Docker stack. Discovery runs over the
host's Tailscale tailnet (primary) and the local LAN via mDNS (fallback). The
LLM agent itself is the peer — a pasted setup prompt teaches it to install the
binary, advertise its agent card, and use a small CLI to send / receive A2A
tasks.

## Goals

- One static Rust binary deployable on any cloud or laptop device.
- Implements the [Agent2Agent (A2A) protocol](https://a2a-protocol.org/) on the
  wire (JSON-RPC 2.0 over HTTP, agent card at `/.well-known/agent.json`,
  `tasks/send` and `tasks/get`).
- Auto-advertises and auto-discovers peers — no configuration of peer lists.
- LLM-agnostic: the only interface the LLM needs is a shell.
- Strip everything from the existing repo that does not serve this scope
  (Conduit, Matrix SDK, PyO3 bindings, bundled agents, Python CLI, Docker
  Compose stack).

## Non-goals (v1)

- No streaming (`tasks/sendSubscribe` / SSE).
- No push-notification webhooks.
- No file or binary parts (text parts only).
- No OAuth, JWT, or per-call signing — trust is the tailnet/LAN boundary.
- No GUI, no web dashboard.
- No Docker images. Plain binary + systemd unit only.

These are deliberate cuts. Each is a follow-up issue, not part of v1.

## Architecture

```
                ┌─────────────────────── tailnet (or LAN) ────────────────────────┐
                │                                                                  │
  ┌──────────────────────┐                                ┌──────────────────────┐ │
  │       Device A       │                                │       Device B       │ │
  │ ┌──────────────────┐ │       JSON-RPC 2.0 / HTTP      │ ┌──────────────────┐ │ │
  │ │ LLM coding agent │ │◄──────────────────────────────►│ │ LLM coding agent │ │ │
  │ │  (Claude Code,   │ │                                │ │  (Claude Code,   │ │ │
  │ │   Cursor, ...)   │ │                                │ │   Cursor, ...)   │ │ │
  │ └────────┬─────────┘ │                                │ └────────┬─────────┘ │ │
  │          │ shell     │                                │          │ shell     │ │
  │          ▼           │                                │          ▼           │ │
  │  ┌───────────────┐   │                                │  ┌───────────────┐   │ │
  │  │ constellation │   │                                │  │ constellation │   │ │
  │  │   (binary)    │   │                                │  │   (binary)    │   │ │
  │  └───────┬───────┘   │                                │  └───────┬───────┘   │ │
  │          │           │                                │          │           │ │
  │   100.x.x.x:7777     │     A2A peer-to-peer calls     │   100.x.x.y:7777     │ │
  └──────────┼───────────┘◄──────────────────────────────►└──────────┼───────────┘ │
             │                                                       │             │
             └───────────── discovery: tailscale + mDNS ──────────────┘             │
                                                                                    │
└──────────────────────────────────────────────────────────────────────────────────┘
```

A peer is any device running `constellation serve`. It binds an HTTP server to
its tailnet IP (or LAN IP), serves an agent card, and exposes the A2A JSON-RPC
endpoint. Discovery populates a local peer cache. The LLM agent on the same
device drives outbound work and reads inbound work through the CLI.

## Components

Each module has one purpose, one set of public types, and is independently
testable.

### `crates/constellation-a2a` — A2A wire types

- Serde models for `AgentCard`, `Skill`, `Task`, `Message`, `Part`, `Artifact`,
  the `tasks/send` and `tasks/get` JSON-RPC request/response shapes.
- No I/O — pure types and (de)serialization, with conformance unit tests
  against fixture JSON copied from `a2a-protocol.org`.
- Reusable as a library by other Rust A2A projects.

### `crates/constellation-discovery` — Peer discovery

- `Discoverer` trait: `async fn poll(&self) -> Vec<DiscoveredPeer>`.
- Two impls:
  - `TailscaleDiscoverer` — shells `tailscale status --json`, filters
    online peers, probes `http://<peer-ip>:7777/.well-known/agent.json`,
    returns the peers that respond with a valid card.
  - `MdnsDiscoverer` — advertises `_a2a._tcp.local` with TXT record
    `name=<agent>`; browses for the same service on the LAN; probes each
    candidate's agent card.
- Errors are non-fatal — a peer that fails to respond is dropped from the
  result, never blocks discovery.
- Discovery runs on a 30s interval inside `serve`; results are written to the
  peer cache.

### `crates/constellation-store` — Local persistence

- SQLite via `rusqlite` (bundled feature). Single file at
  `$XDG_DATA_HOME/constellation/store.db`.
- Three tables:
  - `peers(id, name, tailscale_ip, port, card_json, last_seen)`
  - `tasks_in(task_id, from_peer, state, payload_json, response_json,
    created_at, updated_at)` — tasks sent **to** us.
  - `tasks_out(task_id, to_peer, state, payload_json, response_json,
    created_at, updated_at)` — tasks **we** sent.
- Functions, not a manager: `upsert_peer`, `list_peers`, `insert_in_task`,
  `update_in_task_response`, etc. No abstract repository layer.

### `crates/constellation-server` — A2A HTTP server

- `axum` server bound to `0.0.0.0:7777` (or configured port).
- Routes:
  - `GET /.well-known/agent.json` → renders agent card from config.
  - `POST /` → JSON-RPC 2.0 dispatcher. Implements `tasks/send`,
    `tasks/get`, `tasks/cancel` per A2A spec. Persists inbound tasks to
    `tasks_in` in `submitted` state.
- No execution. The LLM agent picks tasks up via CLI and writes responses
  back. The server transitions task state when CLI marks them complete.

### `crates/constellation-client` — A2A HTTP client

- `reqwest`-based client. One public function per RPC method:
  `send_task(peer, message) -> Task`, `get_task(peer, id) -> Task`.
- Used by the CLI's `send` and `wait` subcommands.

### `crates/constellation-cli` — Binary entry point

- `clap` subcommands. Single binary `constellation`.
- Reads/writes config at `$XDG_CONFIG_HOME/constellation/config.toml`.
- Subcommands:

| Command                             | Purpose                                                                                                                                       |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `constellation init`                | Interactive: ask for agent name, skills, port. Write config. Generate agent card. Print the LLM setup prompt.                                 |
| `constellation serve`               | Run discovery loop + A2A HTTP server. Foreground; meant to be supervised by systemd or `nohup`.                                               |
| `constellation peers`               | List currently-known peers (name, tailscale IP, skills) from the store.                                                                       |
| `constellation send <peer> <text>` | Send a task to `<peer>` (matched by name). Prints task id. Returns immediately.                                                                |
| `constellation wait <task-id>`      | Block until the outbound task reaches `completed` or `failed`. Prints final response.                                                          |
| `constellation inbox`               | Print inbound tasks awaiting a response, oldest first. Each line: `<task-id>  <from-peer>  <one-line preview>`. JSON output via `--json`.     |
| `constellation respond <id> <text>` | Mark inbound task `id` as `completed` with `text` as the response message. Server publishes the response on the next `tasks/get` from peer. |
| `constellation card`                | Print the local agent card JSON (debug aid).                                                                                                  |
| `constellation install-service`     | Install a systemd user unit that runs `constellation serve`. Optional helper.                                                                 |

### Setup prompt template

A single Markdown file, `docs/setup-prompt.md`, embedded in the binary via
`include_str!`. `constellation init` prints it to stdout. The user copies it
into the LLM agent (Claude Code, Cursor, etc.) once. The prompt:

1. Tells the LLM the device is part of an A2A mesh.
2. Explains the agent's identity and skills (filled in from config).
3. Documents the `constellation` CLI verbs the LLM is permitted to call.
4. Defines the LLM's contract:
   - Periodically call `constellation inbox`.
   - For each inbound task, decide whether the device's skills cover it; if
     yes, do the work locally and call `constellation respond`. If no, call
     `constellation peers`, pick a better peer by skill, send via
     `constellation send`, then `wait`, then forward the answer.
5. Includes worked examples.

The prompt is the only "intelligence" in the loop. The Rust binary is
plumbing.

## Data flow — outbound task

1. User asks LLM agent A: "research X."
2. LLM A inspects local skills; not a fit. Runs `constellation peers`,
   identifies B has the `research` skill.
3. LLM A: `constellation send B "research X"`. CLI inserts a row in
   `tasks_out`, calls `tasks/send` on B, stores the task id returned.
4. B's server persists to `tasks_in` (`submitted`).
5. B's LLM, on next inbox poll, sees the task. Performs research.
6. B's LLM: `constellation respond <id> "X is ..."`. Local store flips state
   to `completed`.
7. A's LLM: `constellation wait <task-id>` polls B's `tasks/get` every 2s.
   When B reports `completed`, A prints the response.
8. A's LLM hands the response to its user.

## Configuration (`config.toml`)

```toml
[agent]
name = "atmos-vnic"            # also the A2A agent card name
description = "Cloud ARM dev box"
skills = ["bash", "github", "docker", "research"]

[network]
bind = "0.0.0.0:7777"
advertised_host = "auto"       # "auto" → tailscale IP if available, else LAN IP
discovery = ["tailscale", "mdns"]

[store]
path = "auto"                  # auto → $XDG_DATA_HOME/constellation/store.db
```

`advertised_host = "auto"` resolves at startup by:

1. Run `tailscale ip -4`. If it returns an address, use it.
2. Otherwise, pick the first non-loopback IPv4 of an up interface.
3. Otherwise, refuse to start with a clear error.

## Security boundary

- **Trust model:** the tailnet (or LAN) is the security boundary. Anyone on
  it can call any peer's A2A endpoint.
- **Network defaults:** `bind` defaults to `0.0.0.0:7777` so that the binary
  is reachable on the chosen interface; this is intentional. If the user is
  not on a tailnet and their LAN is hostile, they must change `bind` to
  `127.0.0.1` or a Tailscale-only interface IP.
- **Input validation:** all task message bodies are treated as untrusted
  text. The Rust binary never executes them — the LLM agent decides what
  to do with the content. The binary's only side effect is SQLite writes
  and outbound HTTP.
- **No secrets in agent cards.** The card lists agent name, host, and
  skill labels only.
- **Dependency hygiene:** no `unsafe` in our crates; deny lints on
  `unsafe_code`. Run `cargo audit` in CI.

A `SECURITY.md` will document this trust model and how to harden it
(bind to Tailscale-only IP, rotate Tailscale auth keys, remove peers).

## Testing strategy

- **Unit tests** in every crate. `constellation-a2a` validates
  serialization round-trips against fixture JSON.
- **Integration tests** in `tests/`: spawn two `constellation serve`
  processes on `127.0.0.1` with different ports, set their store paths to
  tempdirs, exchange a task, assert the response arrives. Uses the mDNS
  discoverer in loopback mode.
- **CI**: existing `.github/workflows/ci.yml` is rewritten — `cargo fmt
  --check`, `cargo clippy -D warnings`, `cargo test`, `cargo audit`.

## Strip plan (files removed in this rewrite)

- `conduit/` (entire dir)
- `agents/` (entire dir — coordinator, researcher, coder, common.py,
  base.Dockerfile)
- `cli/constellation_cli.py`
- `examples/simple_agent.py`, `examples/multi_agent_demo.py`
- `sdk/constellation-core/`, `sdk/constellation-py/` (current Matrix SDK)
- `docker-compose.yml`, `docker-compose.prod.yml`, `.dockerignore`
- `scripts/setup.sh`, `scripts/register-agents.sh`
- `Makefile` (replaced with `cargo` workflow)
- `tests/integration/` content — replaced with new Rust integration tests

## Files added or rewritten

- `Cargo.toml` (workspace) at repo root
- `crates/constellation-a2a/`
- `crates/constellation-discovery/`
- `crates/constellation-store/`
- `crates/constellation-server/`
- `crates/constellation-client/`
- `crates/constellation-cli/`
- `docs/setup-prompt.md`
- `docs/SECURITY.md`
- `scripts/health-check.sh` (rewritten — pings local `/.well-known/agent.json`)
- `scripts/security-check.sh` (rewritten — `cargo audit` + bind / config check)
- `.github/workflows/ci.yml` (rewritten for Rust workspace)
- `README.md` (rewritten for the new model)

## Out of scope for v1 (explicit follow-ups)

- Streaming (`tasks/sendSubscribe` over SSE).
- Push notifications via webhook.
- Authentication (mutual TLS, JWT, OAuth).
- Capability-based skill routing (today the LLM picks the peer; a future
  router could short-circuit pure data tasks without LLM mediation).
- A2A files / artifacts beyond text.
- MCP wrapper around the same daemon (option C from the design discussion).

## Open questions resolved

- Q1 — Protocol: official A2A spec.
- Q2 — Topology: peer-to-peer; no central server; tailnet + mDNS for
  reachability and discovery.
- Q3 — Peer identity: the LLM coding agent itself is the peer. The Rust
  binary is a thin runtime + CLI bridge. Agent uses shell tool calls to
  drive it.
