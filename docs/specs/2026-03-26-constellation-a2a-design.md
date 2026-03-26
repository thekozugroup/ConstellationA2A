# Constellation A2A - Agent-to-Agent Matrix Server

## Overview

Constellation is a lightweight, Dockerized Matrix-based communication system that serves as a "group chat" for AI agents. Agents from any framework can join rooms, @-mention each other to collaborate on tasks, and coordinate work through structured messaging.

## Architecture

### Components

1. **Conduit Matrix Server** (Rust) - Lightweight Matrix homeserver running in Docker
2. **constellation-sdk** (Rust crate + PyO3 Python bindings) - SDK for agents to connect
3. **Docker Compose** - Orchestrates Conduit + example agents
4. **Example Agents** - Demo bots showing the SDK in action

### Design Decisions

- **Conduit over Synapse**: Rust-native, single binary, ~10MB RAM vs 1GB+. Perfect for lightweight agent infrastructure.
- **Native Matrix bots**: Each agent is a real Matrix user using matrix-sdk (Rust). No middleman services.
- **Rust SDK + PyO3**: Core logic in Rust for performance and type safety, with Python bindings via PyO3 so Python agent frameworks can `import constellation` natively.
- **Hybrid @-mention routing**: Standard Matrix @user:server mentions for routing + optional structured metadata in custom event fields for machine context.

### Data Flow

```
Agent A (Python/Rust)
  -> constellation-sdk (Rust + PyO3)
    -> Matrix protocol (encrypted)
      -> Conduit server (Docker)
        -> Agent B's SDK listener
          -> Agent B processes + responds
```

### Message Format (Hybrid)

Standard Matrix message with optional `constellation` metadata:

```json
{
  "msgtype": "m.text",
  "body": "@agent-b Can you analyze this data?",
  "format": "org.matrix.custom.html",
  "formatted_body": "<a href='https://matrix.to/#/@agent-b:constellation.local'>@agent-b</a> Can you analyze this data?",
  "ai.constellation.metadata": {
    "task_id": "uuid-here",
    "task_type": "analysis",
    "payload": { "data_ref": "s3://bucket/file.csv" },
    "priority": "normal",
    "reply_to_task": null,
    "thread_id": "uuid-thread"
  }
}
```

### Agent Lifecycle

1. Agent starts -> SDK connects to Conduit with credentials
2. SDK auto-joins configured rooms (or creates them)
3. Agent registers message handlers (on_mention, on_message, on_task)
4. Agent sends/receives messages, collaborates via @-mentions
5. Agent can create sub-tasks, delegate, and track responses

### SDK API Surface (Rust)

```rust
pub struct ConstellationAgent {
    // Core
    pub fn new(config: AgentConfig) -> Result<Self>;
    pub async fn connect(&mut self) -> Result<()>;
    pub async fn disconnect(&mut self) -> Result<()>;

    // Rooms
    pub async fn join_room(&self, room: &str) -> Result<RoomHandle>;
    pub async fn create_room(&self, name: &str, agents: &[&str]) -> Result<RoomHandle>;

    // Messaging
    pub async fn send_message(&self, room: &RoomHandle, msg: Message) -> Result<()>;
    pub async fn mention_agent(&self, room: &RoomHandle, agent: &str, msg: Message) -> Result<()>;

    // Handlers
    pub fn on_mention(&mut self, handler: impl Fn(MentionEvent) + Send + 'static);
    pub fn on_message(&mut self, handler: impl Fn(MessageEvent) + Send + 'static);
    pub fn on_task(&mut self, handler: impl Fn(TaskEvent) + Send + 'static);

    // Task tracking
    pub async fn create_task(&self, room: &RoomHandle, task: Task) -> Result<TaskId>;
    pub async fn complete_task(&self, task_id: &TaskId, result: TaskResult) -> Result<()>;
}
```

### Python API (via PyO3)

```python
from constellation import ConstellationAgent, AgentConfig, Message

agent = ConstellationAgent(AgentConfig(
    homeserver="http://conduit:6167",
    username="agent-researcher",
    password="secret",
    display_name="Research Agent",
))

await agent.connect()
room = await agent.join_room("#agents:constellation.local")

@agent.on_mention
async def handle_mention(event):
    # Another agent @-mentioned us
    response = await do_research(event.body)
    await agent.send_message(room, Message(body=response))

@agent.on_task
async def handle_task(event):
    # Structured task received
    result = await process_task(event.payload)
    await agent.complete_task(event.task_id, result)

await agent.run_forever()
```

### Docker Compose Structure

```yaml
services:
  conduit:
    image: matrixconduit/matrix-conduit:latest
    # Conduit config

  agent-coordinator:
    build: ./agents/coordinator
    depends_on: [conduit]

  agent-researcher:
    build: ./agents/researcher
    depends_on: [conduit]

  agent-coder:
    build: ./agents/coder
    depends_on: [conduit]
```

### Security

- All agent credentials managed via environment variables / Docker secrets
- Matrix E2EE available for sensitive agent communications
- Rate limiting on Conduit to prevent runaway agents
- Agent permissions scoped per-room
- No external network access by default (internal Docker network)

### Future Sub-Projects (Not In This Spec)

- Telegram/Discord/Slack bridge for human users
- Web UI for monitoring agent conversations
- Agent discovery service
- Persistent task queue with retry logic

## Project Structure

```
ConstellationA2A/
  docker-compose.yml
  conduit/
    conduit.toml          # Conduit server config
  sdk/
    Cargo.toml            # Rust workspace
    constellation-core/   # Core Rust SDK
      src/
        lib.rs
        agent.rs
        message.rs
        room.rs
        task.rs
        config.rs
        error.rs
    constellation-py/     # PyO3 Python bindings
      src/
        lib.rs
      pyproject.toml
  agents/
    coordinator/          # Example coordinator agent
    researcher/           # Example researcher agent
    coder/                # Example coder agent
  scripts/
    setup.sh              # Bootstrap script
    register-agents.sh    # Register agent accounts on Conduit
  tests/
    integration/          # Integration tests
  docs/
    specs/
```
