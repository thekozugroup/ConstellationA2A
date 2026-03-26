# Contributing to Constellation A2A

## Development Setup

### Prerequisites

- Docker and Docker Compose
- Rust toolchain (1.77+) via [rustup](https://rustup.rs/)
- Python 3.11+
- [maturin](https://github.com/PyO3/maturin) (`pip install maturin`)

### Getting Started

```bash
# Clone the repository
git clone <repo-url>
cd Constellation

# Copy environment config
cp .env.example .env

# Run initial setup (builds Conduit, generates registration secret)
make setup

# Start all services
make up

# Verify everything is running
make health
make status
```

## Adding a New Agent

1. Create the agent directory:

```bash
mkdir agents/<agent-name>
```

2. Create `agents/<agent-name>/agent.py` following the standard pattern:

```python
"""<Agent Name> - Brief description."""

import asyncio
import logging
import os
import signal
import sys

from constellation import ConstellationAgent, AgentConfig, Message

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
)
log = logging.getLogger("<agent-name>")


async def main():
    config = AgentConfig(
        homeserver=os.environ["MATRIX_HOMESERVER"],
        username=os.environ["AGENT_USERNAME"],
        password=os.environ["AGENT_PASSWORD"],
        display_name=os.environ.get("AGENT_DISPLAY_NAME", "<Agent Name>"),
    )

    agent = ConstellationAgent(config)
    shutdown_event = asyncio.Event()

    def handle_signal():
        log.info("Shutdown signal received, stopping gracefully...")
        shutdown_event.set()

    loop = asyncio.get_running_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, handle_signal)

    await agent.connect()

    rooms_env = os.environ.get("AUTO_JOIN_ROOMS", "")
    room = None
    for room_alias in rooms_env.split(","):
        room_alias = room_alias.strip()
        if room_alias:
            room = await agent.join_room(room_alias)

    @agent.on_mention
    async def handle_mention(event):
        # Your agent logic here
        await agent.send_message(room, Message(body="Response"))

    @agent.on_task
    async def handle_task(event):
        # Structured task handling
        result = {}  # Your processing
        await agent.complete_task(event.task_id, result)

    await shutdown_event.wait()
    await agent.disconnect()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
    sys.exit(0)
```

3. Create `agents/<agent-name>/Dockerfile` (copy from an existing agent and change the COPY path), or use the shared `agents/base.Dockerfile` via docker-compose.

4. Add the service to `docker-compose.yml`:

```yaml
  agent-<name>:
    build:
      context: .
      dockerfile: agents/base.Dockerfile
    depends_on:
      conduit:
        condition: service_healthy
    environment:
      - MATRIX_HOMESERVER=http://conduit:6167
      - AGENT_USERNAME=<name>
      - AGENT_PASSWORD=${<NAME>_PASSWORD:-<name>-secret}
      - AGENT_DISPLAY_NAME=<Display Name>
      - AUTO_JOIN_ROOMS=#constellation:constellation.local
    volumes:
      - ./agents/<name>/agent.py:/app/agent.py:ro
    networks:
      - constellation
```

5. Add the agent's password variable to `.env.example` and `.env`.

6. Register the agent account via `make register` (or add it to `scripts/register-agents.sh`).

## SDK Development

The SDK is a Rust workspace at `sdk/` with two crates:

- `constellation-core` - Core Rust SDK (agent, messaging, rooms, tasks)
- `constellation-py` - PyO3 Python bindings

### Building

```bash
# Build the Rust SDK
make sdk-build

# Run Rust tests
make sdk-test

# Build Python bindings for local development
make sdk-python

# After sdk-python, you can import in Python:
python -c "from constellation import ConstellationAgent"
```

### Workflow

1. Make changes to `constellation-core/src/` for core functionality.
2. Expose new functionality in `constellation-py/src/lib.rs` via PyO3.
3. Run `make sdk-test` to verify Rust tests pass.
4. Run `make sdk-python` to rebuild the Python wheel.
5. Test with an agent script locally or via `make up`.

### Key Files

```
sdk/
  Cargo.toml                      # Workspace root
  constellation-core/
    src/lib.rs                     # Crate root, re-exports
    src/agent.rs                   # ConstellationAgent struct
    src/config.rs                  # AgentConfig
    src/message.rs                 # Message types
    src/room.rs                    # RoomHandle
    src/task.rs                    # Task, TaskId, TaskResult
    src/error.rs                   # Error types
  constellation-py/
    Cargo.toml                     # PyO3 crate config
    pyproject.toml                 # Maturin build config
    src/lib.rs                     # Python bindings
```

## Testing

### Integration Tests

```bash
# Start services first
make up

# Run integration tests
make test
```

Integration tests live in `tests/integration/` and verify end-to-end agent communication through the Conduit server.

### SDK Unit Tests

```bash
make sdk-test
```

### Manual Testing

```bash
# Start services
make up

# Watch agent logs
make logs

# Open a shell in Conduit to inspect state
make shell-conduit
```

## Code Style

### Rust

- Format with `cargo fmt`
- Lint with `cargo clippy`

```bash
cd sdk
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

### Python

- Format with [black](https://github.com/psf/black)
- Lint with [ruff](https://github.com/astral-sh/ruff)

```bash
black agents/
ruff check agents/
```

### General

- Keep agent scripts focused -- one responsibility per agent.
- Use environment variables for all configuration, never hardcode credentials.
- All agents must handle SIGTERM for graceful shutdown.
- Use structured logging (`logging` module) with consistent format across agents.

## Pull Request Process

1. Create a feature branch from `main`:
   ```bash
   git checkout -b feature/my-change
   ```

2. Make your changes and verify:
   - `cargo fmt --all` and `cargo clippy` pass (for Rust changes)
   - `black` and `ruff` pass (for Python changes)
   - `make sdk-test` passes
   - `make up` starts successfully
   - `make test` passes (if integration tests exist for your change)

3. Commit with a clear message describing the change.

4. Open a pull request against `main` with:
   - Summary of what changed and why
   - Testing steps to verify the change
   - Any breaking changes noted

5. Address review feedback and ensure CI passes before merging.
