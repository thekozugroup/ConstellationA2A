"""Coordinator Agent - Smart task router with multi-step pipeline support."""

import time
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from constellation import Message
from common import BaseAgent, TaskInfo, generate_task_id


AGENT_REGISTRY = {
    "researcher": {
        "description": "Research, analysis, information gathering, summarization",
        "keywords": [
            "research", "find", "look up", "search", "analyze", "analysis",
            "summarize", "explain", "what is", "how does", "compare", "review",
            "investigate", "explore", "study", "gather", "information",
        ],
        "status": "unknown",
        "last_seen": None,
    },
    "coder": {
        "description": "Code generation, debugging, refactoring, implementation",
        "keywords": [
            "code", "implement", "function", "bug", "fix", "program", "script",
            "class", "refactor", "write", "create", "build", "generate", "debug",
            "test", "python", "rust", "javascript", "typescript", "api",
        ],
        "status": "unknown",
        "last_seen": None,
    },
}

# Keywords that signal a multi-step task
CHAIN_KEYWORDS = {
    "then": True,
    "and then": True,
    "after that": True,
    "followed by": True,
    "next": True,
}


class CoordinatorAgent(BaseAgent):
    def __init__(self):
        super().__init__("coordinator", "Coordinator Agent")
        self.active_tasks: dict[str, TaskInfo] = {}
        self.completed_tasks: list[TaskInfo] = []

    def register_handlers(self):
        @self.agent.on_mention
        async def handle_mention(event):
            body = event.body.strip()
            sender = event.sender
            self.log.info("Received mention from %s: %s", sender, body)

            # Strip own @-mention from body if present
            # Handle both @coordinator and @coordinator:server.name formats
            clean_body = body
            for prefix in (
                f"@{self.config.username}:{self.server_name}",
                f"@coordinator:{self.server_name}",
                f"@{self.config.username}",
                f"@coordinator",
            ):
                if clean_body.lower().startswith(prefix.lower()):
                    clean_body = clean_body[len(prefix):].strip().lstrip(":").strip()
                    break

            cmd = clean_body.lower()

            if cmd in ("help", "?", "help me"):
                await self._send_help()
                return

            if cmd in ("status", "status?"):
                await self._send_status()
                return

            if cmd.startswith("ping "):
                agent_name = cmd[5:].strip()
                await self._ping_agent(agent_name)
                return

            # Check for multi-step task
            chain = self._parse_chain(clean_body)
            if chain and len(chain) > 1:
                await self._handle_chain(chain, sender)
            else:
                await self._handle_single_task(clean_body, sender)

        @self.agent.on_message
        async def handle_message(event):
            if event.sender == f"@{self.config.username}:{self.server_name}":
                return

            sender_name = event.sender.split(":")[0].lstrip("@")

            # Track agent activity
            if sender_name in AGENT_REGISTRY:
                AGENT_REGISTRY[sender_name]["status"] = "online"
                AGENT_REGISTRY[sender_name]["last_seen"] = time.time()

            # Check if this is a task completion
            reply_task = getattr(event, "reply_to_task", None)

            if reply_task and reply_task in self.active_tasks:
                task = self.active_tasks[reply_task]
                task.status = "completed"
                self.log.info("Task %s completed by %s", reply_task, sender_name)

                # Handle chain: dispatch next step
                if task.chain_next and task.chain_agent:
                    next_task_id = generate_task_id()
                    next_description = task.chain_next
                    next_agent = task.chain_agent

                    self.log.info(
                        "Chain continues: %s -> %s (task %s)",
                        next_agent, next_description, next_task_id,
                    )

                    next_task = TaskInfo(
                        task_id=next_task_id,
                        description=next_description,
                        assigned_to=next_agent,
                        requested_by=task.requested_by,
                    )
                    self.active_tasks[next_task_id] = next_task

                    context = event.body if hasattr(event, "body") else ""
                    delegation_body = (
                        f"@{next_agent} [Task {next_task_id}] {next_description}\n\n"
                        f"Context from previous step:\n{context}"
                    )

                    await self.agent.mention_agent(
                        self.room, next_agent,
                        Message(body=delegation_body),
                    )

                # Move to completed
                del self.active_tasks[reply_task]
                self.completed_tasks.append(task)
                if len(self.completed_tasks) > 50:
                    self.completed_tasks = self.completed_tasks[-50:]

    def _classify_task(self, body: str) -> str:
        """Route a task to the best agent based on keyword scoring."""
        body_lower = body.lower()
        scores: dict[str, int] = {}

        for agent_name, info in AGENT_REGISTRY.items():
            score = sum(1 for kw in info["keywords"] if kw in body_lower)
            scores[agent_name] = score

        best = max(scores, key=scores.get)
        if scores[best] == 0:
            return "researcher"  # default fallback
        return best

    def _parse_chain(self, body: str) -> list[tuple[str, str]] | None:
        """Parse multi-step tasks like 'research X then code Y'.

        Returns list of (agent, description) tuples, or None if not a chain.
        """
        body_lower = body.lower()

        # Find the best splitting keyword
        split_keyword = None
        split_pos = -1
        for kw in CHAIN_KEYWORDS:
            pos = body_lower.find(f" {kw} ")
            if pos != -1 and (split_pos == -1 or pos < split_pos):
                split_keyword = kw
                split_pos = pos

        if split_keyword is None:
            return None

        part1 = body[:split_pos].strip()
        part2 = body[split_pos + len(split_keyword) + 2:].strip()

        if not part1 or not part2:
            return None

        agent1 = self._classify_task(part1)
        agent2 = self._classify_task(part2)

        return [(agent1, part1), (agent2, part2)]

    async def _handle_single_task(self, body: str, sender: str):
        """Classify and delegate a single task."""
        target_agent = self._classify_task(body)
        task_id = generate_task_id()

        task = TaskInfo(
            task_id=task_id,
            description=body,
            assigned_to=target_agent,
            requested_by=sender,
        )
        self.active_tasks[task_id] = task

        self.log.info("Delegating task %s to %s: %s", task_id, target_agent, body)

        delegation_body = f"@{target_agent} [Task {task_id}] {body}"
        await self.agent.mention_agent(
            self.room, target_agent,
            Message(body=delegation_body),
        )

        ack = f"Got it. Delegated to **@{target_agent}** (task {task_id})."
        await self.agent.send_message(self.room, Message(body=ack))

    async def _handle_chain(self, chain: list[tuple[str, str]], sender: str):
        """Handle a multi-step task chain."""
        # Build chain from last to first so we can link them
        task_ids = []
        for i, (agent, desc) in enumerate(chain):
            task_ids.append(generate_task_id())

        # Create task infos with chain links
        for i, (agent, desc) in enumerate(chain):
            task = TaskInfo(
                task_id=task_ids[i],
                description=desc,
                assigned_to=agent,
                requested_by=sender,
            )
            if i < len(chain) - 1:
                task.chain_next = chain[i + 1][1]
                task.chain_agent = chain[i + 1][0]
            task_ids[i] = task.task_id
            self.active_tasks[task.task_id] = task

        # Start the first step
        first_agent, first_desc = chain[0]
        first_task = list(self.active_tasks.values())[-len(chain)]

        self.log.info(
            "Starting %d-step chain. Step 1: %s -> %s",
            len(chain), first_agent, first_desc,
        )

        steps_desc = " -> ".join(
            f"**@{agent}**: {desc}" for agent, desc in chain
        )

        ack = (
            f"Multi-step task detected ({len(chain)} steps):\n"
            f"{steps_desc}\n\n"
            f"Starting with step 1..."
        )
        await self.agent.send_message(self.room, Message(body=ack))

        delegation_body = f"@{first_agent} [Task {first_task.task_id}] {first_desc}"
        await self.agent.mention_agent(
            self.room, first_agent,
            Message(body=delegation_body),
        )

    async def _send_help(self):
        """Send the help message listing available agents and commands."""
        lines = [
            "**Coordinator Agent** - I route tasks to specialist agents.",
            "",
            "**Commands:**",
            "  - `help` - Show this help message",
            "  - `status` - Show agent status and active tasks",
            "  - `ping <agent>` - Check if an agent is responsive",
            "",
            "**Available Agents:**",
        ]

        for name, info in AGENT_REGISTRY.items():
            status = info["status"]
            lines.append(f"  - **@{name}** ({status}): {info['description']}")

        lines.extend([
            "",
            "**Usage:**",
            "  - Send me a task and I'll route it to the best agent.",
            "  - For multi-step tasks, use 'then': *research X then code Y*",
        ])

        await self.agent.send_message(self.room, Message(body="\n".join(lines)))

    async def _send_status(self):
        """Send status report of agents and tasks."""
        lines = ["**System Status**", ""]

        # Agent status
        lines.append("**Agents:**")
        for name, info in AGENT_REGISTRY.items():
            status = info["status"]
            last_seen = info["last_seen"]
            if last_seen:
                ago = int(time.time() - last_seen)
                if ago < 60:
                    seen_str = f"{ago}s ago"
                elif ago < 3600:
                    seen_str = f"{ago // 60}m ago"
                else:
                    seen_str = f"{ago // 3600}h ago"
                lines.append(f"  - **@{name}**: {status} (last seen {seen_str})")
            else:
                lines.append(f"  - **@{name}**: {status}")

        # Active tasks
        lines.append("")
        if self.active_tasks:
            lines.append(f"**Active Tasks ({len(self.active_tasks)}):**")
            for tid, task in self.active_tasks.items():
                age = int(time.time() - task.created_at)
                lines.append(
                    f"  - `{tid}` -> @{task.assigned_to}: "
                    f"{task.description[:60]} ({age}s)"
                )
                if task.chain_next:
                    lines.append(f"    Next: @{task.chain_agent} -> {task.chain_next[:40]}")
        else:
            lines.append("**Active Tasks:** None")

        # Recent completed
        if self.completed_tasks:
            recent = self.completed_tasks[-5:]
            lines.append("")
            lines.append(f"**Recently Completed ({len(recent)}/{len(self.completed_tasks)}):**")
            for task in recent:
                lines.append(
                    f"  - `{task.task_id}` @{task.assigned_to}: "
                    f"{task.description[:60]}"
                )

        await self.agent.send_message(self.room, Message(body="\n".join(lines)))

    async def _ping_agent(self, agent_name: str):
        """Ping an agent to check responsiveness."""
        if agent_name not in AGENT_REGISTRY:
            await self.agent.send_message(
                self.room,
                Message(body=f"Unknown agent: **{agent_name}**. "
                        f"Available: {', '.join(AGENT_REGISTRY.keys())}"),
            )
            return

        info = AGENT_REGISTRY[agent_name]
        if info["last_seen"]:
            ago = int(time.time() - info["last_seen"])
            await self.agent.send_message(
                self.room,
                Message(body=f"@{agent_name} was last seen {ago}s ago. "
                        f"Status: {info['status']}"),
            )
        else:
            await self.agent.send_message(
                self.room,
                Message(body=f"@{agent_name} has not been seen yet. "
                        f"Status: {info['status']}"),
            )


if __name__ == "__main__":
    CoordinatorAgent().run()
