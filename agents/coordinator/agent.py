"""Coordinator Agent - Routes tasks to specialist agents."""

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
log = logging.getLogger("coordinator")

AVAILABLE_AGENTS = {
    "researcher": "Research Agent - web research, data analysis, information gathering",
    "coder": "Coder Agent - code generation, debugging, code review",
}

HELP_TEXT = (
    "I'm the Coordinator Agent. I route tasks to specialist agents.\n\n"
    "Available agents:\n"
    + "\n".join(f"  - @{name}: {desc}" for name, desc in AVAILABLE_AGENTS.items())
    + "\n\nSend me a task and I'll delegate it to the right agent."
)


def classify_task(body: str) -> str:
    """Simple keyword-based task classification."""
    body_lower = body.lower()
    code_keywords = ["code", "implement", "function", "bug", "fix", "program", "script", "class", "refactor"]
    if any(kw in body_lower for kw in code_keywords):
        return "coder"
    return "researcher"


async def main():
    config = AgentConfig(
        homeserver=os.environ["MATRIX_HOMESERVER"],
        username=os.environ["AGENT_USERNAME"],
        password=os.environ["AGENT_PASSWORD"],
        display_name=os.environ.get("AGENT_DISPLAY_NAME", "Coordinator Agent"),
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
    log.info("Coordinator agent connected to Matrix.")

    rooms_env = os.environ.get("AUTO_JOIN_ROOMS", "")
    room = None
    for room_alias in rooms_env.split(","):
        room_alias = room_alias.strip()
        if room_alias:
            room = await agent.join_room(room_alias)
            log.info("Joined room: %s", room_alias)

    active_tasks: dict[str, str] = {}  # task_id -> delegated_agent

    @agent.on_mention
    async def handle_mention(event):
        body = event.body.strip()
        log.info("Received mention from %s: %s", event.sender, body)

        if body.lower() in ("help", "?", "help me"):
            await agent.send_message(room, Message(body=HELP_TEXT))
            return

        target_agent = classify_task(body)
        log.info("Delegating task to %s", target_agent)

        delegation_msg = f"@{target_agent} {body}"
        await agent.mention_agent(room, target_agent, Message(body=delegation_msg))

        if hasattr(event, "task_id") and event.task_id:
            active_tasks[event.task_id] = target_agent
            log.info("Tracking task %s -> %s", event.task_id, target_agent)

    @agent.on_message
    async def handle_message(event):
        if event.sender == config.username:
            return

        sender_name = event.sender.split(":")[0].lstrip("@")
        if sender_name in AVAILABLE_AGENTS:
            task_id = getattr(event, "reply_to_task", None)
            if task_id and task_id in active_tasks:
                del active_tasks[task_id]
                log.info("Task %s completed by %s", task_id, sender_name)

    log.info("Coordinator agent running. Waiting for messages...")

    await shutdown_event.wait()
    await agent.disconnect()
    log.info("Coordinator agent stopped.")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
    sys.exit(0)
