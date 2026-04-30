# Constellation A2A

**Peer-to-peer Agent2Agent (A2A) mesh over Tailscale.**

[![ci](https://github.com/thekozugroup/ConstellationA2A/actions/workflows/ci.yml/badge.svg)](https://github.com/thekozugroup/ConstellationA2A/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Constellation turns any LLM coding agent (Claude Code, Cursor, Codex, …)
into a peer on a private agent mesh. There is no central server. Each
device runs a single Rust binary that speaks the
[A2A protocol](https://a2a-protocol.org/) over JSON-RPC and discovers
other peers automatically — over Tailscale (primary) or mDNS on the local
network (fallback).

```
                ┌───────────── tailnet (or LAN) ─────────────┐
   Device A   ◄────────── A2A JSON-RPC / HTTP ──────────►   Device B
   constellation                                            constellation
   + your LLM agent                                         + your LLM agent
```

## How it works

1. You run `constellation serve` on each device. It binds an HTTP server
   on your tailnet IP (or LAN IP), publishes an
   [agent card](https://a2a-protocol.org/#agent-cards) at
   `/.well-known/agent.json`, and starts discovering peers.
2. You paste the prompt printed by `constellation init` into your LLM
   coding agent — once. The prompt teaches the LLM how to use the
   `constellation` CLI to send tasks, receive tasks, and respond.
3. The LLM drives the mesh through five shell verbs: `peers`, `send`,
   `wait`, `inbox`, `respond`. The Rust binary is plumbing; the LLM is
   the intelligence.

## Quickstart

```bash
# 1) Build & install the binary.
cargo install --path crates/constellation-cli

# 2) Configure this node and copy the prompt that gets pasted into your LLM.
constellation init --name my-box --skills bash,research

# 3) Run as a systemd user service.
constellation install-service
systemctl --user enable --now constellation

# 4) Verify.
constellation card | jq .name
constellation peers
```

## CLI verbs

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

`$XDG_CONFIG_HOME/constellation/config.toml` (created by `constellation init`):

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

## Project layout

| Crate                                | Responsibility                              |
| ------------------------------------ | ------------------------------------------- |
| `crates/constellation-a2a`           | A2A protocol wire types (no I/O).           |
| `crates/constellation-store`         | SQLite persistence (peers + tasks).         |
| `crates/constellation-discovery`     | Tailscale + mDNS peer discovery.            |
| `crates/constellation-server`        | Axum HTTP A2A JSON-RPC server.              |
| `crates/constellation-client`        | A2A JSON-RPC client.                        |
| `crates/constellation-cli`           | `constellation` binary entry point.         |

## A2A scope (v1)

- Implemented: `tasks/send`, `tasks/get`, `/.well-known/agent.json`.
- Out of scope (planned): streaming via SSE, push notifications, file or
  binary parts, OAuth/JWT, `tasks/cancel`.

## Discovery

- **Tailscale (primary)** — `tailscale status --json` enumerates online
  peers; each is probed for an agent card.
- **mDNS (LAN fallback)** — service type `_a2a._tcp.local.`; advertised
  on startup and browsed continuously.

Set `[network] discovery = ["tailscale"]` to disable LAN discovery on
hosts where the LAN is not trusted.

## Security

Constellation trusts its transport. Run it on a Tailscale tailnet and
limit `discovery` accordingly. See [`docs/SECURITY.md`](docs/SECURITY.md).

## Development

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

CI is defined in `.github/workflows/ci.yml`.

## License

[MIT](LICENSE) — Copyright 2026 Tachyon Labs HQ.
