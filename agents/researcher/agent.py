"""Research Agent - Performs research and returns structured findings."""

import re
import time
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from constellation import Message
from common import BaseAgent, generate_task_id


# Knowledge base for mock research
KNOWLEDGE_BASE = {
    "api": {
        "title": "API Design Patterns",
        "sections": {
            "REST": [
                "Standard CRUD operations with HTTP verbs",
                "JSON payloads, stateless, cacheable",
                "Best for: public APIs, simple CRUD, web clients",
            ],
            "GraphQL": [
                "Flexible queries, client-driven data fetching",
                "Single endpoint, strong typing with schema",
                "Best for: complex data relationships, mobile apps",
            ],
            "gRPC": [
                "High performance binary protocol (protobuf)",
                "Bidirectional streaming, strong typing",
                "Best for: inter-service communication, low latency",
            ],
        },
        "recommendation": "REST for simplicity, gRPC for internal services, GraphQL for complex client needs",
    },
    "database": {
        "title": "Database Technologies",
        "sections": {
            "Relational": [
                "PostgreSQL: Full-featured, ACID, extensible, best general choice",
                "MySQL: Widely deployed, good read performance",
                "SQLite: Embedded, zero-config, great for local/testing",
            ],
            "Document": [
                "MongoDB: Flexible schema, horizontal scaling",
                "CouchDB: HTTP API, offline-first sync",
            ],
            "Key-Value": [
                "Redis: In-memory, pub/sub, caching, queues",
                "etcd: Distributed config, service discovery",
            ],
        },
        "recommendation": "PostgreSQL for most use cases, Redis for caching, MongoDB for schema flexibility",
    },
    "auth": {
        "title": "Authentication & Authorization",
        "sections": {
            "Standards": [
                "OAuth 2.0: Industry standard for delegated authorization",
                "OpenID Connect: Identity layer on top of OAuth 2.0",
                "SAML: Enterprise SSO, XML-based",
            ],
            "Token Types": [
                "JWT: Self-contained, stateless, good for APIs",
                "Opaque tokens: Server-side validation, revocable",
                "Session cookies: Traditional web apps, CSRF protection needed",
            ],
            "Best Practices": [
                "Use short-lived access tokens with refresh tokens",
                "Hash passwords with bcrypt/argon2, never store plaintext",
                "Implement rate limiting on auth endpoints",
            ],
        },
        "recommendation": "OAuth 2.0 + JWT for APIs, OpenID Connect for user-facing apps",
    },
    "testing": {
        "title": "Testing Strategies",
        "sections": {
            "Unit Tests": [
                "Test individual functions/methods in isolation",
                "Fast, deterministic, high coverage target (>80%)",
                "Mock external dependencies",
            ],
            "Integration Tests": [
                "Test component interactions with real dependencies",
                "Database, API, message queue integration",
                "Slower but catches interface bugs",
            ],
            "E2E Tests": [
                "Full system tests simulating user workflows",
                "Selenium/Playwright for web, API test suites for services",
                "Expensive, run on CI, keep suite small and focused",
            ],
        },
        "recommendation": "Testing pyramid: many unit, some integration, few E2E",
    },
    "architecture": {
        "title": "Software Architecture Patterns",
        "sections": {
            "Monolith": [
                "Single deployable unit, simpler operations",
                "Good starting point, easier debugging",
                "Scale vertically, modularize internally",
            ],
            "Microservices": [
                "Independent deployment, technology diversity",
                "Requires service mesh, distributed tracing",
                "Only when team/scale demands it",
            ],
            "Event-Driven": [
                "Loosely coupled via events/messages",
                "Kafka, RabbitMQ, NATS for message brokers",
                "Good for async workflows and data pipelines",
            ],
        },
        "recommendation": "Start monolith, extract services only when complexity demands it",
    },
}


class ResearcherAgent(BaseAgent):
    def __init__(self):
        super().__init__("researcher", "Research Agent")
        self.research_history: list[dict] = []

    def register_handlers(self):
        @self.agent.on_mention
        async def handle_mention(event):
            body = event.body.strip()
            sender = event.sender
            self.log.info("Research request from %s: %s", sender, body)

            # Strip own @-mention from body (handles @user:server format)
            clean_body = body
            for prefix in (
                f"@{self.config.username}:{self.server_name}",
                f"@researcher:{self.server_name}",
                f"@{self.config.username}",
                "@researcher",
            ):
                if clean_body.lower().startswith(prefix.lower()):
                    clean_body = clean_body[len(prefix):].strip().lstrip(":").strip()
                    break

            # Extract task ID if present
            task_id = None
            task_match = re.match(r"\[Task (task-[a-f0-9]+)\]\s*(.*)", clean_body, re.DOTALL)
            if task_match:
                task_id = task_match.group(1)
                clean_body = task_match.group(2).strip()

            # Perform research
            findings = self._do_research(clean_body)

            # Track history
            entry = {
                "query": clean_body,
                "sender": sender,
                "task_id": task_id,
                "timestamp": time.time(),
                "topic": findings["title"],
            }
            self.research_history.append(entry)
            if len(self.research_history) > 100:
                self.research_history = self.research_history[-100:]

            # Format and send response
            response = self._format_findings(findings, task_id)
            await self.agent.send_message(self.room, Message(body=response))
            self.log.info("Sent research findings on: %s", findings["title"])

            # If implementation is needed, mention coder
            if self._needs_implementation(clean_body):
                self.log.info("Research suggests implementation needed, mentioning coder.")
                coder_msg = (
                    f"@coder Based on research findings above, "
                    f"please implement: {clean_body}"
                )
                if task_id:
                    coder_msg = f"@coder [Task {task_id}] {coder_msg[7:]}"
                await self.agent.mention_agent(
                    self.room, "coder",
                    Message(body=coder_msg),
                )

        @self.agent.on_task
        async def handle_task(event):
            self.log.info("Received structured task: %s", event.task_id)
            query = event.payload.get("query", "")
            findings = self._do_research(query)
            response = self._format_findings(findings, event.task_id)
            await self.agent.complete_task(event.task_id, {"findings": response})
            self.log.info("Task %s completed.", event.task_id)

    def _do_research(self, query: str) -> dict:
        """Look up research from the knowledge base."""
        query_lower = query.lower()

        # Score each topic by keyword matches
        best_topic = None
        best_score = 0

        for keyword, data in KNOWLEDGE_BASE.items():
            score = 0
            if keyword in query_lower:
                score += 10
            # Check section titles too
            for section_title in data["sections"]:
                if section_title.lower() in query_lower:
                    score += 5
            # Check content items
            for items in data["sections"].values():
                for item in items:
                    words = set(item.lower().split())
                    query_words = set(query_lower.split())
                    score += len(words & query_words)

            if score > best_score:
                best_score = score
                best_topic = keyword

        if best_topic and best_score > 0:
            return KNOWLEDGE_BASE[best_topic]

        # Default: return a generic response
        return {
            "title": "General Research",
            "sections": {
                "Analysis": [
                    f"Analyzed query: '{query}'",
                    "Searched available knowledge base for relevant information",
                    "No exact match found - consider refining the query",
                ],
                "Suggestions": [
                    "Try keywords: api, database, auth, testing, architecture",
                    "Be specific about the technology or pattern you need",
                    "Ask the coordinator for help routing your request",
                ],
            },
            "recommendation": "Refine the query with specific technology keywords for better results",
        }

    def _format_findings(self, findings: dict, task_id: str | None = None) -> str:
        """Format research findings into a structured response."""
        lines = []

        if task_id:
            lines.append(f"**Research Report** (Task: `{task_id}`)")
        else:
            lines.append("**Research Report**")

        lines.append(f"**Topic:** {findings['title']}")
        lines.append("")

        for section_title, items in findings["sections"].items():
            lines.append(f"**{section_title}:**")
            for item in items:
                lines.append(f"  - {item}")
            lines.append("")

        if findings.get("recommendation"):
            lines.append(f"**Recommendation:** {findings['recommendation']}")

        lines.append("")
        lines.append(f"---\n*Research history: {len(self.research_history)} queries processed*")

        return "\n".join(lines)

    def _needs_implementation(self, query: str) -> bool:
        """Determine if the research suggests code should be written."""
        signals = [
            "implement", "build", "create a", "write code", "code example",
            "make a", "develop", "set up", "configure",
        ]
        return any(s in query.lower() for s in signals)


if __name__ == "__main__":
    ResearcherAgent().run()
