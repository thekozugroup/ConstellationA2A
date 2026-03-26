"""Coder Agent - Generates code in multiple languages from requests."""

import re
import time
import sys
import os

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from constellation import Message
from common import BaseAgent, generate_task_id


# Language-specific code templates
TEMPLATES = {
    "python": {
        "api": {
            "description": "FastAPI REST endpoint",
            "code": '''\
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

app = FastAPI(title="Constellation API")


class Item(BaseModel):
    name: str
    description: str | None = None
    status: str = "active"


items_db: dict[int, Item] = {}
next_id = 1


@app.get("/items/{item_id}")
async def get_item(item_id: int) -> Item:
    if item_id not in items_db:
        raise HTTPException(status_code=404, detail="Item not found")
    return items_db[item_id]


@app.post("/items", status_code=201)
async def create_item(item: Item) -> dict:
    global next_id
    items_db[next_id] = item
    result = {"id": next_id, **item.model_dump()}
    next_id += 1
    return result


@app.get("/items")
async def list_items(status: str | None = None) -> list[dict]:
    results = []
    for id_, item in items_db.items():
        if status is None or item.status == status:
            results.append({"id": id_, **item.model_dump()})
    return results''',
        },
        "function": {
            "description": "Data processing function",
            "code": '''\
from typing import Any


def process_data(records: list[dict[str, Any]]) -> dict[str, Any]:
    """Process a list of records and return aggregated results."""
    if not records:
        return {"count": 0, "summary": "No data provided"}

    values = [r.get("value", 0) for r in records]
    total = sum(values)
    count = len(records)

    return {
        "count": count,
        "total": total,
        "average": round(total / count, 2) if count else 0,
        "min": min(values),
        "max": max(values),
        "summary": f"Processed {count} records (total: {total})",
    }''',
        },
        "class": {
            "description": "Task manager class",
            "code": '''\
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum


class TaskStatus(Enum):
    PENDING = "pending"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"
    FAILED = "failed"


@dataclass
class Task:
    id: str
    description: str
    status: TaskStatus = TaskStatus.PENDING
    created_at: datetime = field(default_factory=datetime.now)
    completed_at: datetime | None = None


class TaskManager:
    """Manages tasks with lifecycle tracking."""

    def __init__(self):
        self._tasks: dict[str, Task] = {}

    def create(self, task_id: str, description: str) -> Task:
        if task_id in self._tasks:
            raise ValueError(f"Task {task_id} already exists")
        task = Task(id=task_id, description=description)
        self._tasks[task_id] = task
        return task

    def start(self, task_id: str) -> Task:
        task = self._get(task_id)
        task.status = TaskStatus.IN_PROGRESS
        return task

    def complete(self, task_id: str) -> Task:
        task = self._get(task_id)
        task.status = TaskStatus.COMPLETED
        task.completed_at = datetime.now()
        return task

    def fail(self, task_id: str) -> Task:
        task = self._get(task_id)
        task.status = TaskStatus.FAILED
        return task

    def list_by_status(self, status: TaskStatus | None = None) -> list[Task]:
        if status is None:
            return list(self._tasks.values())
        return [t for t in self._tasks.values() if t.status == status]

    def _get(self, task_id: str) -> Task:
        if task_id not in self._tasks:
            raise KeyError(f"Task {task_id} not found")
        return self._tasks[task_id]''',
        },
        "test": {
            "description": "pytest test suite",
            "code": '''\
import pytest


class TestExample:
    """Example test suite."""

    def test_basic_operation(self):
        result = 2 + 2
        assert result == 4

    def test_string_processing(self):
        data = "hello world"
        assert data.upper() == "HELLO WORLD"
        assert data.split() == ["hello", "world"]

    def test_list_operations(self):
        items = [3, 1, 4, 1, 5, 9]
        assert sorted(items) == [1, 1, 3, 4, 5, 9]
        assert len(items) == 6

    @pytest.mark.parametrize("input_val,expected", [
        (0, 0),
        (1, 1),
        (5, 120),
        (10, 3628800),
    ])
    def test_factorial(self, input_val, expected):
        def factorial(n):
            if n <= 1:
                return max(n, 0)
            return n * factorial(n - 1)
        assert factorial(input_val) == expected''',
        },
    },
    "rust": {
        "struct": {
            "description": "Rust struct with implementation",
            "code": '''\
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Config {
    values: HashMap<String, String>,
}

impl Config {
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }

    pub fn get_or_default<'a>(&'a self, key: &str, default: &'a str) -> &'a str {
        self.get(key).unwrap_or(default)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.values.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}''',
        },
        "api": {
            "description": "Axum web handler",
            "code": '''\
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
    id: u64,
    name: String,
    status: String,
}

type AppState = Arc<RwLock<Vec<Item>>>;

async fn list_items(State(state): State<AppState>) -> Json<Vec<Item>> {
    let items = state.read().await;
    Json(items.clone())
}

async fn get_item(
    Path(id): Path<u64>,
    State(state): State<AppState>,
) -> Result<Json<Item>, StatusCode> {
    let items = state.read().await;
    items
        .iter()
        .find(|item| item.id == id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/items", get(list_items))
        .route("/items/{id}", get(get_item))
}''',
        },
    },
    "javascript": {
        "api": {
            "description": "Express.js REST endpoint",
            "code": '''\
const express = require('express');
const router = express.Router();

const items = new Map();
let nextId = 1;

router.get('/items', (req, res) => {
  const { status } = req.query;
  let results = [...items.values()];
  if (status) {
    results = results.filter(item => item.status === status);
  }
  res.json(results);
});

router.get('/items/:id', (req, res) => {
  const item = items.get(parseInt(req.params.id));
  if (!item) {
    return res.status(404).json({ error: 'Item not found' });
  }
  res.json(item);
});

router.post('/items', (req, res) => {
  const { name, description } = req.body;
  if (!name) {
    return res.status(400).json({ error: 'Name is required' });
  }
  const item = {
    id: nextId++,
    name,
    description: description || null,
    status: 'active',
    createdAt: new Date().toISOString(),
  };
  items.set(item.id, item);
  res.status(201).json(item);
});

module.exports = router;''',
        },
        "function": {
            "description": "Utility function",
            "code": '''\
/**
 * Process an array of records and return aggregated results.
 * @param {Array<{value: number}>} records
 * @returns {{count: number, total: number, average: number, min: number, max: number}}
 */
function processData(records) {
  if (!records || records.length === 0) {
    return { count: 0, total: 0, average: 0, min: 0, max: 0 };
  }

  const values = records.map(r => r.value || 0);
  const total = values.reduce((sum, v) => sum + v, 0);

  return {
    count: records.length,
    total,
    average: Math.round((total / records.length) * 100) / 100,
    min: Math.min(...values),
    max: Math.max(...values),
  };
}

module.exports = { processData };''',
        },
    },
}

# Detect which language the user wants
LANGUAGE_KEYWORDS = {
    "python": ["python", "py", "fastapi", "django", "flask", "pytest"],
    "rust": ["rust", "cargo", "axum", "tokio", "struct", "impl"],
    "javascript": ["javascript", "js", "node", "express", "react", "npm"],
}

# Detect which template type the user wants
TYPE_KEYWORDS = {
    "api": ["api", "endpoint", "rest", "route", "handler", "server", "http"],
    "function": ["function", "process", "utility", "helper", "transform", "data"],
    "class": ["class", "manager", "service", "model", "object"],
    "struct": ["struct", "type", "data structure"],
    "test": ["test", "spec", "assert", "pytest", "unittest"],
}


class CoderAgent(BaseAgent):
    def __init__(self):
        super().__init__("coder", "Coder Agent")
        self.code_history: list[dict] = []

    def register_handlers(self):
        @self.agent.on_mention
        async def handle_mention(event):
            body = event.body.strip()
            sender = event.sender
            self.log.info("Code request from %s: %s", sender, body)

            # Strip own @-mention
            clean_body = body
            for prefix in (f"@{self.config.username}", "@coder"):
                if clean_body.lower().startswith(prefix.lower()):
                    clean_body = clean_body[len(prefix):].strip()
                    break

            # Extract task ID
            task_id = None
            task_match = re.match(r"\[Task (task-[a-f0-9]+)\]\s*(.*)", clean_body, re.DOTALL)
            if task_match:
                task_id = task_match.group(1)
                clean_body = task_match.group(2).strip()

            # Check if more context is needed
            if self._needs_research(clean_body):
                self.log.info("Need more context, asking researcher.")
                researcher_msg = (
                    f"@researcher I need context before coding: {clean_body}"
                )
                await self.agent.mention_agent(
                    self.room, "researcher",
                    Message(body=researcher_msg),
                )
                await self.agent.send_message(
                    self.room,
                    Message(body="I need some research context first. "
                            "Asked @researcher for background information."),
                )
                return

            # Generate code
            language, template_type, code, description = self._generate_code(clean_body)

            # Track history
            entry = {
                "query": clean_body,
                "sender": sender,
                "task_id": task_id,
                "language": language,
                "type": template_type,
                "timestamp": time.time(),
            }
            self.code_history.append(entry)
            if len(self.code_history) > 100:
                self.code_history = self.code_history[-100:]

            # Format response
            response = self._format_response(
                code, language, description, task_id,
            )
            await self.agent.send_message(self.room, Message(body=response))
            self.log.info("Sent %s code (%s)", language, template_type)

        @self.agent.on_task
        async def handle_task(event):
            self.log.info("Received structured task: %s", event.task_id)
            description = event.payload.get("description", "")
            language, template_type, code, desc = self._generate_code(description)
            response = self._format_response(code, language, desc, event.task_id)
            await self.agent.complete_task(event.task_id, {"code": response})
            self.log.info("Task %s completed.", event.task_id)

    def _detect_language(self, query: str) -> str:
        """Detect the desired programming language from the query."""
        query_lower = query.lower()
        scores: dict[str, int] = {}

        for lang, keywords in LANGUAGE_KEYWORDS.items():
            scores[lang] = sum(1 for kw in keywords if kw in query_lower)

        best = max(scores, key=scores.get)
        if scores[best] > 0:
            return best
        return "python"  # default

    def _detect_type(self, query: str, language: str) -> str:
        """Detect the desired code template type."""
        query_lower = query.lower()
        scores: dict[str, int] = {}

        for type_name, keywords in TYPE_KEYWORDS.items():
            scores[type_name] = sum(1 for kw in keywords if kw in query_lower)

        best = max(scores, key=scores.get)
        if scores[best] > 0:
            # Check if this type exists for the language
            if best in TEMPLATES.get(language, {}):
                return best
            # Fall back to first available type
            available = list(TEMPLATES.get(language, {}).keys())
            if available:
                return available[0]
        return "function" if "function" in TEMPLATES.get(language, {}) else list(TEMPLATES.get(language, {}).keys())[0]

    def _generate_code(self, query: str) -> tuple[str, str, str, str]:
        """Generate code based on the query.

        Returns (language, template_type, code, description).
        """
        language = self._detect_language(query)
        template_type = self._detect_type(query, language)

        lang_templates = TEMPLATES.get(language, TEMPLATES["python"])
        template = lang_templates.get(template_type, list(lang_templates.values())[0])

        return language, template_type, template["code"], template["description"]

    def _format_response(
        self, code: str, language: str, description: str,
        task_id: str | None = None,
    ) -> str:
        """Format a code response with metadata."""
        lines = []

        if task_id:
            lines.append(f"**Code Generated** (Task: `{task_id}`)")
        else:
            lines.append("**Code Generated**")

        lines.append(f"**Language:** {language.capitalize()}")
        lines.append(f"**Type:** {description}")
        lines.append("")
        lines.append(f"```{language}")
        lines.append(code)
        lines.append("```")
        lines.append("")
        lines.append(f"---\n*Code history: {len(self.code_history)} snippets generated*")

        return "\n".join(lines)

    def _needs_research(self, query: str) -> bool:
        """Check if the query is too vague and needs research first."""
        vague_signals = [
            "best way to", "what approach", "how should i",
            "recommend", "compare options", "which is better",
        ]
        query_lower = query.lower()
        return any(s in query_lower for s in vague_signals)


if __name__ == "__main__":
    CoderAgent().run()
