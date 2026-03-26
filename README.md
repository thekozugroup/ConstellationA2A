# Constellation A2A

Agent-to-Agent communication over Matrix. A lightweight, Dockerized system where AI agents collaborate through a shared Matrix chat server.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                Docker Network                        │
│                                                      │
│  ┌──────────┐    ┌──────────────────────────────┐   │
│  │ Conduit   │    │       Agent Containers       │   │
│  │ Matrix    │◄──►│                              │   │
│  │ Server    │    │  ┌────────────┐              │   │
│  │           │    │  │Coordinator │──delegates──►│   │
│  │ :6167     │    │  └────────────┘              │   │
│  │           │    │  ┌────────────┐              │   │
│  │           │    │  │ Researcher │              │   │
│  │           │    │  └────────────┘              │   │
│  │           │    │  ┌────────────┐              │   │
│  │           │    │  │   Coder    │              │   │
│  │           │    │  └────────────┘              │   │
│  └──────────┘    └──────────────────────────────┘   │
│       ▲                                              │
│       │ :8448 (host)                                 │
└───────┼──────────────────────────────────────────────┘
        │
   External access
```

**Conduit** is a lightweight Rust Matrix homeserver (~10MB RAM). Agents connect as native Matrix users via the **constellation SDK** (Rust core + Python bindings via PyO3). They communicate through @-mentions and structured task metadata.

## Quick Start

```bash
# 1. Clone and configure
cp .env.example .env
# Edit .env with your own secrets

# 2. Run the setup script (builds Conduit, generates secrets)
./scripts/setup.sh

# 3. Start all services
docker compose up -d

# 4. Check health
curl http://localhost:8448/_matrix/client/versions
```

## Services

| Service            | Description                          | Port  |
|--------------------|--------------------------------------|-------|
| `conduit`          | Matrix homeserver                    | 8448  |
| `agent-coordinator`| Routes tasks to specialist agents    | -     |
| `agent-researcher` | Research and information gathering   | -     |
| `agent-coder`      | Code generation and review           | -     |

## Configuration

Environment variables (set in `.env`):

| Variable               | Default                  | Description                      |
|------------------------|--------------------------|----------------------------------|
| `REGISTRATION_SECRET`  | `change-me-in-production`| Conduit registration secret      |
| `COORDINATOR_PASSWORD` | `coordinator-secret`     | Coordinator agent password       |
| `RESEARCHER_PASSWORD`  | `researcher-secret`      | Researcher agent password        |
| `CODER_PASSWORD`       | `coder-secret`           | Coder agent password             |

## SDK Usage

### Python

```python
from constellation import ConstellationAgent, AgentConfig, Message

agent = ConstellationAgent(AgentConfig(
    homeserver="http://conduit:6167",
    username="my-agent",
    password="secret",
    display_name="My Agent",
))

await agent.connect()
room = await agent.join_room("#constellation:constellation.local")

@agent.on_mention
async def handle_mention(event):
    await agent.send_message(room, Message(body="Hello!"))

await agent.run_forever()
```

### Rust

```rust
use constellation_core::{ConstellationAgent, AgentConfig, Message};

let config = AgentConfig {
    homeserver: "http://conduit:6167".into(),
    username: "my-agent".into(),
    password: "secret".into(),
    display_name: Some("My Agent".into()),
};

let mut agent = ConstellationAgent::new(config)?;
agent.connect().await?;

let room = agent.join_room("#constellation:constellation.local").await?;

agent.on_mention(|event| async move {
    agent.send_message(&room, Message::text("Hello!")).await?;
    Ok(())
});

agent.run_forever().await?;
```

## Development

```bash
# Build just Conduit
docker compose build conduit

# Build all agents (uses shared base.Dockerfile)
docker compose build

# View logs
docker compose logs -f agent-coordinator

# Restart a single agent
docker compose restart agent-researcher

# Stop everything
docker compose down

# Stop and remove volumes
docker compose down -v
```

### Adding a New Agent

1. Create `agents/<name>/agent.py` using the constellation SDK
2. Create `agents/<name>/Dockerfile` (copy from an existing agent) or use the shared `base.Dockerfile`
3. Add the service to `docker-compose.yml`
4. Register the agent account via the setup script

### Project Structure

```
Constellation/
  docker-compose.yml          # Service orchestration
  .env.example                # Environment template
  conduit/
    Dockerfile                # Conduit Matrix server image
    conduit.toml              # Server configuration
  sdk/
    Cargo.toml                # Rust workspace
    constellation-core/       # Core Rust SDK
    constellation-py/         # PyO3 Python bindings
  agents/
    base.Dockerfile           # Shared multi-stage build
    coordinator/              # Task routing agent
    researcher/               # Research agent
    coder/                    # Code generation agent
  scripts/
    setup.sh                  # Bootstrap script
  docs/
    specs/                    # Design specifications
```

## License

MIT
