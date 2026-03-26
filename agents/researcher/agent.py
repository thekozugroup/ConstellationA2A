"""Research Agent - Performs research tasks and returns findings."""

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
log = logging.getLogger("researcher")

# Mock research database for demonstration
MOCK_RESEARCH = {
    "default": (
        "Research findings:\n"
        "- Analyzed the topic using available sources\n"
        "- Found 3 relevant papers and 5 documentation references\n"
        "- Key insight: the consensus approach is well-documented\n"
        "- Confidence: moderate (mock data)"
    ),
    "api": (
        "API Research Results:\n"
        "- REST API: Standard CRUD with JSON payloads\n"
        "- GraphQL: Flexible queries, good for complex data relationships\n"
        "- gRPC: High performance, binary protocol, strong typing\n"
        "- Recommendation: REST for simplicity, gRPC for inter-service"
    ),
    "database": (
        "Database Research Results:\n"
        "- PostgreSQL: Best for relational data with ACID compliance\n"
        "- MongoDB: Document store, flexible schema\n"
        "- Redis: In-memory cache, pub/sub support\n"
        "- SQLite: Embedded, zero-config, great for local/testing"
    ),
}


def do_research(query: str) -> str:
    """Simulate research by matching keywords to mock results."""
    query_lower = query.lower()
    for keyword, result in MOCK_RESEARCH.items():
        if keyword in query_lower:
            return result
    return MOCK_RESEARCH["default"]


def needs_code(query: str) -> bool:
    """Determine if the research result suggests code is needed."""
    code_signals = ["implement", "build", "create a", "write code", "code example"]
    return any(s in query.lower() for s in code_signals)


async def main():
    config = AgentConfig(
        homeserver=os.environ["MATRIX_HOMESERVER"],
        username=os.environ["AGENT_USERNAME"],
        password=os.environ["AGENT_PASSWORD"],
        display_name=os.environ.get("AGENT_DISPLAY_NAME", "Research Agent"),
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
    log.info("Research agent connected to Matrix.")

    rooms_env = os.environ.get("AUTO_JOIN_ROOMS", "")
    room = None
    for room_alias in rooms_env.split(","):
        room_alias = room_alias.strip()
        if room_alias:
            room = await agent.join_room(room_alias)
            log.info("Joined room: %s", room_alias)

    @agent.on_mention
    async def handle_mention(event):
        body = event.body.strip()
        log.info("Research request from %s: %s", event.sender, body)

        result = do_research(body)
        await agent.send_message(room, Message(body=result))
        log.info("Sent research results.")

        if needs_code(body):
            log.info("Research suggests code is needed, delegating to coder.")
            await agent.mention_agent(
                room,
                "coder",
                Message(body=f"@coder Based on research, please {body}"),
            )

    @agent.on_task
    async def handle_task(event):
        log.info("Received structured task: %s", event.task_id)
        result = do_research(event.payload.get("query", ""))
        await agent.complete_task(event.task_id, {"findings": result})
        log.info("Task %s completed.", event.task_id)

    log.info("Research agent running. Waiting for messages...")

    await shutdown_event.wait()
    await agent.disconnect()
    log.info("Research agent stopped.")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
    sys.exit(0)
