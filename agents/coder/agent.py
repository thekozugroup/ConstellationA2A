"""Coder Agent - Generates code snippets and performs code tasks."""

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
log = logging.getLogger("coder")

# Mock code templates for demonstration
CODE_TEMPLATES = {
    "api": '''"""Example REST API endpoint."""
from fastapi import FastAPI, HTTPException

app = FastAPI()

@app.get("/items/{item_id}")
async def get_item(item_id: int):
    # TODO: Replace with actual database lookup
    return {"item_id": item_id, "name": "Example Item", "status": "active"}

@app.post("/items")
async def create_item(name: str):
    # TODO: Replace with actual database insert
    return {"item_id": 1, "name": name, "status": "created"}
''',
    "function": '''def process_data(data: list[dict]) -> dict:
    """Process a list of records and return aggregated results."""
    if not data:
        return {"count": 0, "summary": "No data provided"}

    total = sum(record.get("value", 0) for record in data)
    return {
        "count": len(data),
        "total": total,
        "average": total / len(data),
        "summary": f"Processed {len(data)} records",
    }
''',
    "class": '''class TaskManager:
    """Manages a collection of tasks with basic CRUD operations."""

    def __init__(self):
        self._tasks: dict[str, dict] = {}

    def create(self, task_id: str, description: str) -> dict:
        task = {"id": task_id, "description": description, "status": "pending"}
        self._tasks[task_id] = task
        return task

    def complete(self, task_id: str) -> dict:
        if task_id not in self._tasks:
            raise KeyError(f"Task {task_id} not found")
        self._tasks[task_id]["status"] = "completed"
        return self._tasks[task_id]

    def list_all(self) -> list[dict]:
        return list(self._tasks.values())
''',
    "default": '''# Generated code snippet
def example():
    """Example function - customize based on your needs."""
    print("Hello from the Coder Agent!")
    return {"status": "ok"}
''',
}


def generate_code(query: str) -> str:
    """Select a code template based on the query keywords."""
    query_lower = query.lower()
    for keyword, template in CODE_TEMPLATES.items():
        if keyword in query_lower:
            return f"Here's the code:\n\n```python\n{template}```"
    return f"Here's a starting point:\n\n```python\n{CODE_TEMPLATES['default']}```"


async def main():
    config = AgentConfig(
        homeserver=os.environ["MATRIX_HOMESERVER"],
        username=os.environ["AGENT_USERNAME"],
        password=os.environ["AGENT_PASSWORD"],
        display_name=os.environ.get("AGENT_DISPLAY_NAME", "Coder Agent"),
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
    log.info("Coder agent connected to Matrix.")

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
        log.info("Code request from %s: %s", event.sender, body)

        result = generate_code(body)
        await agent.send_message(room, Message(body=result))
        log.info("Sent code response.")

    @agent.on_task
    async def handle_task(event):
        log.info("Received structured task: %s", event.task_id)
        code = generate_code(event.payload.get("description", ""))
        await agent.complete_task(event.task_id, {"code": code})
        log.info("Task %s completed.", event.task_id)

    log.info("Coder agent running. Waiting for messages...")

    await shutdown_event.wait()
    await agent.disconnect()
    log.info("Coder agent stopped.")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
    sys.exit(0)
