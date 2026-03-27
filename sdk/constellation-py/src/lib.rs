use std::sync::Arc;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::ToPyObject;
use tokio::sync::Mutex;

use constellation_core::{
    self as core,
    message::{
        Priority as CorePriority, TaskStatus as CoreTaskStatus,
    },
};

// ---------------------------------------------------------------------------
// Helper: convert ConstellationError -> PyErr
// ---------------------------------------------------------------------------

fn to_py_err(e: core::ConstellationError) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

/// Convert a serde_json::Value to a Python object.
fn json_to_py(py: Python<'_>, val: &serde_json::Value) -> PyObject {
    match val {
        serde_json::Value::Null => py.None(),
        serde_json::Value::Bool(b) => b.to_object(py),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_object(py)
            } else if let Some(f) = n.as_f64() {
                f.to_object(py)
            } else {
                py.None()
            }
        }
        serde_json::Value::String(s) => s.to_object(py),
        serde_json::Value::Array(arr) => {
            let list = PyList::empty_bound(py);
            for item in arr {
                list.append(json_to_py(py, item)).unwrap();
            }
            list.to_object(py)
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new_bound(py);
            for (k, v) in map {
                dict.set_item(k, json_to_py(py, v)).unwrap();
            }
            dict.to_object(py)
        }
    }
}

/// Convert a Python object to serde_json::Value.
fn py_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        Ok(serde_json::Value::Null)
    } else if let Ok(b) = obj.extract::<bool>() {
        Ok(serde_json::Value::Bool(b))
    } else if let Ok(i) = obj.extract::<i64>() {
        Ok(serde_json::json!(i))
    } else if let Ok(f) = obj.extract::<f64>() {
        Ok(serde_json::json!(f))
    } else if let Ok(s) = obj.extract::<String>() {
        Ok(serde_json::Value::String(s))
    } else if let Ok(list) = obj.downcast::<PyList>() {
        let mut arr = Vec::new();
        for item in list.iter() {
            arr.push(py_to_json(&item)?);
        }
        Ok(serde_json::Value::Array(arr))
    } else if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key: String = k.extract()?;
            map.insert(key, py_to_json(&v)?);
        }
        Ok(serde_json::Value::Object(map))
    } else {
        Ok(serde_json::Value::Null)
    }
}

// ---------------------------------------------------------------------------
// Priority enum
// ---------------------------------------------------------------------------

/// Task priority level.
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[pymethods]
impl Priority {
    #[new]
    #[pyo3(signature = (value=1))]
    fn new(value: u8) -> PyResult<Self> {
        match value {
            0 => Ok(Priority::Low),
            1 => Ok(Priority::Normal),
            2 => Ok(Priority::High),
            3 => Ok(Priority::Critical),
            _ => Err(PyRuntimeError::new_err("invalid priority value (0-3)")),
        }
    }

    fn __repr__(&self) -> String {
        match self {
            Priority::Low => "Priority.Low".to_string(),
            Priority::Normal => "Priority.Normal".to_string(),
            Priority::High => "Priority.High".to_string(),
            Priority::Critical => "Priority.Critical".to_string(),
        }
    }
}

impl From<Priority> for CorePriority {
    fn from(p: Priority) -> Self {
        match p {
            Priority::Low => CorePriority::Low,
            Priority::Normal => CorePriority::Normal,
            Priority::High => CorePriority::High,
            Priority::Critical => CorePriority::Critical,
        }
    }
}

impl From<CorePriority> for Priority {
    fn from(p: CorePriority) -> Self {
        match p {
            CorePriority::Low => Priority::Low,
            CorePriority::Normal => Priority::Normal,
            CorePriority::High => Priority::High,
            CorePriority::Critical => Priority::Critical,
        }
    }
}

// ---------------------------------------------------------------------------
// TaskStatus enum
// ---------------------------------------------------------------------------

/// Status of a task.
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending = 0,
    InProgress = 1,
    Completed = 2,
    Failed = 3,
}

#[pymethods]
impl TaskStatus {
    fn __repr__(&self) -> String {
        match self {
            TaskStatus::Pending => "TaskStatus.Pending".to_string(),
            TaskStatus::InProgress => "TaskStatus.InProgress".to_string(),
            TaskStatus::Completed => "TaskStatus.Completed".to_string(),
            TaskStatus::Failed => "TaskStatus.Failed".to_string(),
        }
    }
}

impl From<TaskStatus> for CoreTaskStatus {
    fn from(s: TaskStatus) -> Self {
        match s {
            TaskStatus::Pending => CoreTaskStatus::Pending,
            TaskStatus::InProgress => CoreTaskStatus::InProgress,
            TaskStatus::Completed => CoreTaskStatus::Completed,
            TaskStatus::Failed => CoreTaskStatus::Failed,
        }
    }
}

impl From<CoreTaskStatus> for TaskStatus {
    fn from(s: CoreTaskStatus) -> Self {
        match s {
            CoreTaskStatus::Pending => TaskStatus::Pending,
            CoreTaskStatus::InProgress => TaskStatus::InProgress,
            CoreTaskStatus::Completed => TaskStatus::Completed,
            CoreTaskStatus::Failed => TaskStatus::Failed,
        }
    }
}

// ---------------------------------------------------------------------------
// AgentConfig
// ---------------------------------------------------------------------------

/// Configuration for a Constellation agent.
///
/// Example::
///
///     config = AgentConfig(
///         homeserver="http://conduit:6167",
///         username="my-agent",
///         password="secret",
///         display_name="My Agent",
///     )
#[pyclass]
#[derive(Debug, Clone)]
pub struct AgentConfig {
    #[pyo3(get, set)]
    pub homeserver: String,
    #[pyo3(get, set)]
    pub username: String,
    #[pyo3(get, set)]
    pub password: String,
    #[pyo3(get, set)]
    pub display_name: Option<String>,
    #[pyo3(get, set)]
    pub auto_join_rooms: Vec<String>,
}

#[pymethods]
impl AgentConfig {
    #[new]
    #[pyo3(signature = (homeserver, username, password, display_name=None, auto_join_rooms=None))]
    fn new(
        homeserver: String,
        username: String,
        password: String,
        display_name: Option<String>,
        auto_join_rooms: Option<Vec<String>>,
    ) -> Self {
        Self {
            homeserver,
            username,
            password,
            display_name,
            auto_join_rooms: auto_join_rooms.unwrap_or_default(),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "AgentConfig(homeserver={:?}, username={:?}, display_name={:?})",
            self.homeserver, self.username, self.display_name
        )
    }
}

impl From<&AgentConfig> for core::AgentConfig {
    fn from(cfg: &AgentConfig) -> Self {
        core::AgentConfig {
            homeserver_url: cfg.homeserver.clone(),
            username: cfg.username.clone(),
            password: cfg.password.clone(),
            display_name: cfg.display_name.clone(),
            auto_join_rooms: cfg.auto_join_rooms.clone(),
            device_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// A message to send to a room.
///
/// Example::
///
///     msg = Message(body="Hello, world!")
///     msg = Message(body="Task result", metadata={"task_id": "abc"})
#[pyclass]
#[derive(Debug, Clone)]
pub struct Message {
    #[pyo3(get, set)]
    pub body: String,
    /// Optional metadata dict; converted to ConstellationMetadata when sending.
    metadata_json: Option<serde_json::Value>,
}

#[pymethods]
impl Message {
    #[new]
    #[pyo3(signature = (body, metadata=None))]
    fn new(body: String, metadata: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        let metadata_json = match metadata {
            Some(obj) => Some(py_to_json(obj)?),
            None => None,
        };
        Ok(Self { body, metadata_json })
    }

    /// Get the metadata as a Python dict, or None.
    #[getter]
    fn metadata(&self, py: Python<'_>) -> PyObject {
        match &self.metadata_json {
            Some(val) => json_to_py(py, val),
            None => py.None(),
        }
    }

    fn __repr__(&self) -> String {
        format!("Message(body={:?})", self.body)
    }
}

impl Message {
    fn to_core(&self) -> core::message::Message {
        let mut msg = core::message::Message::text(&self.body);
        if let Some(ref val) = self.metadata_json {
            if let Ok(meta) = serde_json::from_value::<core::message::ConstellationMetadata>(val.clone()) {
                msg = msg.with_metadata(meta);
            }
        }
        msg
    }
}

// ---------------------------------------------------------------------------
// RoomHandle
// ---------------------------------------------------------------------------

/// A handle to a joined Matrix room.
#[pyclass]
#[derive(Debug, Clone)]
pub struct RoomHandle {
    inner: core::RoomHandle,
}

#[pymethods]
impl RoomHandle {
    /// The Matrix room ID as a string.
    #[getter]
    fn room_id(&self) -> String {
        self.inner.room_id()
    }

    fn __repr__(&self) -> String {
        format!("RoomHandle(room_id={:?})", self.inner.room_id())
    }
}

// ---------------------------------------------------------------------------
// Event wrapper types exposed to Python callbacks
// ---------------------------------------------------------------------------

/// Event received when this agent is @-mentioned.
#[pyclass]
#[derive(Debug, Clone)]
pub struct MentionEvent {
    #[pyo3(get)]
    pub sender: String,
    #[pyo3(get)]
    pub room_id: String,
    #[pyo3(get)]
    pub body: String,
    #[pyo3(get)]
    pub mentioned_agents: Vec<String>,
    metadata_json: Option<serde_json::Value>,
}

#[pymethods]
impl MentionEvent {
    /// Constellation metadata dict, or None.
    #[getter]
    fn metadata(&self, py: Python<'_>) -> PyObject {
        match &self.metadata_json {
            Some(val) => json_to_py(py, val),
            None => py.None(),
        }
    }

    /// Convenience: extract task_id from metadata if present.
    #[getter]
    fn task_id(&self) -> Option<String> {
        self.metadata_json.as_ref().and_then(|v| {
            v.get("task_id").and_then(|t| t.as_str().map(String::from))
        })
    }
}

impl From<core::message::MentionEvent> for MentionEvent {
    fn from(e: core::message::MentionEvent) -> Self {
        let metadata_json = e.metadata.as_ref().and_then(|m| serde_json::to_value(m).ok());
        Self {
            sender: e.sender,
            room_id: e.room_id,
            body: e.body,
            mentioned_agents: e.mentioned_agents,
            metadata_json,
        }
    }
}

/// Event received for any message in a joined room.
#[pyclass]
#[derive(Debug, Clone)]
pub struct MessageEvent {
    #[pyo3(get)]
    pub sender: String,
    #[pyo3(get)]
    pub room_id: String,
    #[pyo3(get)]
    pub body: String,
    raw_event_json: serde_json::Value,
}

#[pymethods]
impl MessageEvent {
    /// The raw Matrix event as a Python dict.
    #[getter]
    fn raw_event(&self, py: Python<'_>) -> PyObject {
        json_to_py(py, &self.raw_event_json)
    }

    /// Convenience: extract reply_to_task from raw event metadata if present.
    #[getter]
    fn reply_to_task(&self) -> Option<String> {
        self.raw_event_json
            .get("content")
            .and_then(|c| c.get("ai.constellation.metadata"))
            .and_then(|m| m.get("reply_to_task"))
            .and_then(|v| v.as_str().map(String::from))
    }
}

impl From<core::message::MessageEvent> for MessageEvent {
    fn from(e: core::message::MessageEvent) -> Self {
        Self {
            sender: e.sender,
            room_id: e.room_id,
            body: e.body,
            raw_event_json: e.raw_event,
        }
    }
}

/// Event received when a structured task message arrives.
#[pyclass]
#[derive(Debug, Clone)]
pub struct TaskEvent {
    #[pyo3(get)]
    pub sender: String,
    #[pyo3(get)]
    pub room_id: String,
    #[pyo3(get)]
    pub task_id: String,
    #[pyo3(get)]
    pub task_type: String,
    #[pyo3(get)]
    pub priority: Priority,
    payload_json: serde_json::Value,
}

#[pymethods]
impl TaskEvent {
    /// The task payload as a Python dict.
    #[getter]
    fn payload(&self, py: Python<'_>) -> PyObject {
        json_to_py(py, &self.payload_json)
    }
}

impl From<core::message::TaskEvent> for TaskEvent {
    fn from(e: core::message::TaskEvent) -> Self {
        Self {
            sender: e.sender,
            room_id: e.room_id,
            task_id: e.task_id,
            task_type: e.task_type,
            priority: e.priority.into(),
            payload_json: e.payload,
        }
    }
}

// ---------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------

/// A task to create and send to a room.
///
/// Example::
///
///     task = Task(task_type="analysis", payload={"file": "data.csv"})
///     task = Task(task_type="code", payload={}, priority=Priority.High)
#[pyclass]
#[derive(Debug, Clone)]
pub struct Task {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get, set)]
    pub task_type: String,
    #[pyo3(get, set)]
    pub priority: Priority,
    #[pyo3(get, set)]
    pub reply_to_task: Option<String>,
    #[pyo3(get, set)]
    pub thread_id: Option<String>,
    payload_json: serde_json::Value,
}

#[pymethods]
impl Task {
    #[new]
    #[pyo3(signature = (task_type, payload=None, priority=None, reply_to_task=None, thread_id=None))]
    fn new(
        task_type: String,
        payload: Option<&Bound<'_, PyAny>>,
        priority: Option<Priority>,
        reply_to_task: Option<String>,
        thread_id: Option<String>,
    ) -> PyResult<Self> {
        let payload_json = match payload {
            Some(obj) => py_to_json(obj)?,
            None => serde_json::Value::Object(serde_json::Map::new()),
        };
        let core_task = core::message::Task::new(&task_type, payload_json.clone());
        Ok(Self {
            id: core_task.id,
            task_type,
            priority: priority.unwrap_or(Priority::Normal),
            reply_to_task,
            thread_id,
            payload_json,
        })
    }

    /// The task payload as a Python dict.
    #[getter]
    fn payload(&self, py: Python<'_>) -> PyObject {
        json_to_py(py, &self.payload_json)
    }

    fn __repr__(&self) -> String {
        format!("Task(id={:?}, task_type={:?})", self.id, self.task_type)
    }
}

impl Task {
    fn to_core(&self) -> core::message::Task {
        let mut t = core::message::Task::new(&self.task_type, self.payload_json.clone());
        t.id = self.id.clone();
        t.priority = self.priority.into();
        t.reply_to_task = self.reply_to_task.clone();
        t.thread_id = self.thread_id.clone();
        t
    }
}

// ---------------------------------------------------------------------------
// TaskResult
// ---------------------------------------------------------------------------

/// The result of completing a task.
///
/// Example::
///
///     result = TaskResult(task_id="abc", status=TaskStatus.Completed, data={"answer": 42})
#[pyclass]
#[derive(Debug, Clone)]
pub struct TaskResult {
    #[pyo3(get)]
    pub task_id: String,
    #[pyo3(get)]
    pub status: TaskStatus,
    result_data_json: serde_json::Value,
}

#[pymethods]
impl TaskResult {
    #[new]
    #[pyo3(signature = (task_id, status=None, data=None))]
    fn new(
        task_id: String,
        status: Option<TaskStatus>,
        data: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let result_data_json = match data {
            Some(obj) => py_to_json(obj)?,
            None => serde_json::Value::Object(serde_json::Map::new()),
        };
        Ok(Self {
            task_id,
            status: status.unwrap_or(TaskStatus::Completed),
            result_data_json,
        })
    }

    /// The result data as a Python dict.
    #[getter]
    fn data(&self, py: Python<'_>) -> PyObject {
        json_to_py(py, &self.result_data_json)
    }

    fn __repr__(&self) -> String {
        format!("TaskResult(task_id={:?}, status={:?})", self.task_id, self.status)
    }
}

impl TaskResult {
    fn to_core(&self) -> core::message::TaskResult {
        core::message::TaskResult {
            task_id: self.task_id.clone(),
            status: self.status.into(),
            result_data: self.result_data_json.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// ConstellationAgent
// ---------------------------------------------------------------------------

/// The main Constellation agent for connecting to Matrix and communicating
/// with other agents.
///
/// Example::
///
///     agent = ConstellationAgent(AgentConfig(
///         homeserver="http://conduit:6167",
///         username="my-agent",
///         password="secret",
///     ))
///     await agent.connect()
///     room = await agent.join_room("#constellation:constellation.local")
///     await agent.send_message(room, Message(body="Hello!"))
#[pyclass]
pub struct ConstellationAgent {
    inner: Arc<Mutex<core::ConstellationAgent>>,
    config: AgentConfig,
    pending_mention_handlers: Vec<PyObject>,
    pending_message_handlers: Vec<PyObject>,
    pending_task_handlers: Vec<PyObject>,
}

#[pymethods]
impl ConstellationAgent {
    #[new]
    fn new(config: AgentConfig) -> PyResult<Self> {
        let core_config: core::AgentConfig = (&config).into();
        let agent = core::ConstellationAgent::new(core_config).map_err(to_py_err)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(agent)),
            config,
            pending_mention_handlers: Vec::new(),
            pending_message_handlers: Vec::new(),
            pending_task_handlers: Vec::new(),
        })
    }

    /// Connect to the Matrix homeserver.
    ///
    /// This is an awaitable method: ``await agent.connect()``
    fn connect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut agent = inner.lock().await;
            agent.connect().await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Gracefully disconnect from the homeserver.
    fn disconnect<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut agent = inner.lock().await;
            agent.disconnect().await.map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Join a room by alias (e.g. ``"#constellation:constellation.local"``).
    ///
    /// Returns a :class:`RoomHandle`.
    fn join_room<'py>(&self, py: Python<'py>, room_alias: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let agent = inner.lock().await;
            let handle = agent.join_room(&room_alias).await.map_err(to_py_err)?;
            Ok(RoomHandle { inner: handle })
        })
    }

    /// Create a new room, optionally inviting other agents.
    ///
    /// ``agents`` is a list of Matrix user ID strings.
    #[pyo3(signature = (name, agents=None))]
    fn create_room<'py>(
        &self,
        py: Python<'py>,
        name: String,
        agents: Option<Vec<String>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let agent = inner.lock().await;
            let agent_refs: Vec<&str> = agents
                .as_ref()
                .map(|a| a.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default();
            let handle = agent
                .create_room(&name, &agent_refs)
                .await
                .map_err(to_py_err)?;
            Ok(RoomHandle { inner: handle })
        })
    }

    /// Send a message to a room.
    fn send_message<'py>(
        &self,
        py: Python<'py>,
        room: &RoomHandle,
        msg: &Message,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        let room_inner = room.inner.clone();
        let core_msg = msg.to_core();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let agent = inner.lock().await;
            agent
                .send_message(&room_inner, core_msg)
                .await
                .map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Send a message that @-mentions a specific agent.
    ///
    /// ``agent_name`` is the localpart (e.g. ``"researcher"``) or full Matrix
    /// user ID (e.g. ``"@researcher:constellation.local"``).
    fn mention_agent<'py>(
        &self,
        py: Python<'py>,
        room: &RoomHandle,
        agent_name: String,
        msg: &Message,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        let room_inner = room.inner.clone();
        let core_msg = msg.to_core();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let agent = inner.lock().await;
            agent
                .mention_agent(&room_inner, &agent_name, core_msg)
                .await
                .map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Register a callback for when this agent is @-mentioned.
    ///
    /// The callback receives a :class:`MentionEvent` and can be a regular
    /// function or an ``async def``.
    ///
    /// Can also be used as a decorator::
    ///
    ///     @agent.on_mention
    ///     async def handle(event):
    ///         ...
    fn on_mention(&mut self, py: Python<'_>, callback: PyObject) -> PyResult<PyObject> {
        let ret = callback.clone_ref(py);
        self.pending_mention_handlers.push(callback);
        Ok(ret)
    }

    /// Register a callback for all incoming messages.
    ///
    /// The callback receives a :class:`MessageEvent`.
    fn on_message(&mut self, py: Python<'_>, callback: PyObject) -> PyResult<PyObject> {
        let ret = callback.clone_ref(py);
        self.pending_message_handlers.push(callback);
        Ok(ret)
    }

    /// Register a callback for structured task events.
    ///
    /// The callback receives a :class:`TaskEvent`.
    fn on_task(&mut self, py: Python<'_>, callback: PyObject) -> PyResult<PyObject> {
        let ret = callback.clone_ref(py);
        self.pending_task_handlers.push(callback);
        Ok(ret)
    }

    /// Create a task, send it to a room, and start tracking it.
    ///
    /// Returns the task ID string.
    fn create_task<'py>(
        &self,
        py: Python<'py>,
        room: &RoomHandle,
        task: &Task,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        let room_inner = room.inner.clone();
        let core_task = task.to_core();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let agent = inner.lock().await;
            let task_id = agent
                .create_task(&room_inner, core_task)
                .await
                .map_err(to_py_err)?;
            Ok(task_id)
        })
    }

    /// Mark a task as completed and send the result.
    ///
    /// ``result`` can be a :class:`TaskResult` or a plain dict (which will be
    /// wrapped as a successful completion).
    fn complete_task<'py>(
        &self,
        py: Python<'py>,
        task_id: String,
        result: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();

        // Accept either a TaskResult or a plain dict.
        let core_result = if let Ok(tr) = result.extract::<TaskResult>() {
            tr.to_core()
        } else {
            // Treat as a dict payload -> success result.
            let data = py_to_json(result)?;
            core::message::TaskResult::success(&task_id, data)
        };

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let agent = inner.lock().await;
            agent
                .complete_task(&task_id, core_result)
                .await
                .map_err(to_py_err)?;
            Ok(())
        })
    }

    /// Start the sync loop. This blocks until :meth:`disconnect` is called.
    ///
    /// Equivalent to ``await agent.run_forever()``.
    fn run_forever<'py>(&mut self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();

        // Drain pending handlers and move them into the async block
        let mention_cbs: Vec<PyObject> = self.pending_mention_handlers.drain(..).collect();
        let message_cbs: Vec<PyObject> = self.pending_message_handlers.drain(..).collect();
        let task_cbs: Vec<PyObject> = self.pending_task_handlers.drain(..).collect();

        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            // Now we're inside the Tokio runtime — register all handlers
            {
                let agent = inner.lock().await;
                for cb in mention_cbs {
                    agent.on_mention(move |core_event| {
                        let py_event: MentionEvent = core_event.into();
                        Python::with_gil(|py| {
                            let event_obj = match Py::new(py, py_event) {
                                Ok(obj) => obj,
                                Err(e) => { eprintln!("Error creating MentionEvent: {e}"); return; }
                            };
                            let result = cb.call1(py, (event_obj,));
                            if let Ok(awaitable) = result {
                                if awaitable.bind(py).getattr("__await__").is_ok() {
                                    if let Ok(fut) = pyo3_async_runtimes::tokio::into_future(awaitable.bind(py).clone()) {
                                        tokio::spawn(async move { let _ = fut.await; });
                                    }
                                }
                            } else if let Err(e) = result {
                                eprintln!("Error calling on_mention handler: {e}");
                            }
                        });
                    }).await;
                }
                for cb in message_cbs {
                    agent.on_message(move |core_event| {
                        let py_event: MessageEvent = core_event.into();
                        Python::with_gil(|py| {
                            let event_obj = match Py::new(py, py_event) {
                                Ok(obj) => obj,
                                Err(e) => { eprintln!("Error creating MessageEvent: {e}"); return; }
                            };
                            let result = cb.call1(py, (event_obj,));
                            if let Ok(awaitable) = result {
                                if awaitable.bind(py).getattr("__await__").is_ok() {
                                    if let Ok(fut) = pyo3_async_runtimes::tokio::into_future(awaitable.bind(py).clone()) {
                                        tokio::spawn(async move { let _ = fut.await; });
                                    }
                                }
                            } else if let Err(e) = result {
                                eprintln!("Error calling on_message handler: {e}");
                            }
                        });
                    }).await;
                }
                for cb in task_cbs {
                    agent.on_task(move |core_event| {
                        let py_event: TaskEvent = core_event.into();
                        Python::with_gil(|py| {
                            let event_obj = match Py::new(py, py_event) {
                                Ok(obj) => obj,
                                Err(e) => { eprintln!("Error creating TaskEvent: {e}"); return; }
                            };
                            let result = cb.call1(py, (event_obj,));
                            if let Ok(awaitable) = result {
                                if awaitable.bind(py).getattr("__await__").is_ok() {
                                    if let Ok(fut) = pyo3_async_runtimes::tokio::into_future(awaitable.bind(py).clone()) {
                                        tokio::spawn(async move { let _ = fut.await; });
                                    }
                                }
                            } else if let Err(e) = result {
                                eprintln!("Error calling on_task handler: {e}");
                            }
                        });
                    }).await;
                }
            }

            // Now start the sync loop
            let mut agent = inner.lock().await;
            agent.run().await.map_err(to_py_err)?;
            Ok(())
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "ConstellationAgent(username={:?}, homeserver={:?})",
            self.config.username, self.config.homeserver
        )
    }
}

// ---------------------------------------------------------------------------
// Python module definition
// ---------------------------------------------------------------------------

/// Constellation A2A SDK — agent-to-agent communication over Matrix.
#[pymodule]
fn constellation(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", "0.1.0")?;

    // Core types
    m.add_class::<AgentConfig>()?;
    m.add_class::<Message>()?;
    m.add_class::<ConstellationAgent>()?;
    m.add_class::<RoomHandle>()?;

    // Task types
    m.add_class::<Task>()?;
    m.add_class::<TaskResult>()?;

    // Enums
    m.add_class::<Priority>()?;
    m.add_class::<TaskStatus>()?;

    // Event types (for type annotations in user code)
    m.add_class::<MentionEvent>()?;
    m.add_class::<MessageEvent>()?;
    m.add_class::<TaskEvent>()?;

    Ok(())
}
