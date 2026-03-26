use std::sync::Arc;

use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::{
        api::client::room::create_room::v3::Request as CreateRoomRequest,
        events::room::message::{
            MessageType, OriginalSyncRoomMessageEvent,
        },
        OwnedUserId, RoomAliasId,
    },
    Client,
};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

use crate::config::AgentConfig;
use crate::error::{ConstellationError, Result};
use crate::message::{
    parse_mentions, ConstellationMetadata, MentionEvent, Message, MessageEvent, Task, TaskEvent,
    TaskResult,
};
use crate::room::RoomHandle;
use crate::task::TaskManager;

type MentionHandler = Arc<dyn Fn(MentionEvent) + Send + Sync>;
type MessageHandler = Arc<dyn Fn(MessageEvent) + Send + Sync>;
type TaskHandler = Arc<dyn Fn(TaskEvent) + Send + Sync>;

/// The main Constellation agent that wraps a Matrix client.
pub struct ConstellationAgent {
    config: AgentConfig,
    client: Option<Client>,
    user_id: Option<OwnedUserId>,
    mention_handlers: Arc<Mutex<Vec<MentionHandler>>>,
    message_handlers: Arc<Mutex<Vec<MessageHandler>>>,
    task_handlers: Arc<Mutex<Vec<TaskHandler>>>,
    task_manager: Arc<Mutex<TaskManager>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl std::fmt::Debug for ConstellationAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConstellationAgent")
            .field("config", &self.config)
            .field("connected", &self.client.is_some())
            .finish()
    }
}

impl ConstellationAgent {
    /// Create a new agent from the given config.
    pub fn new(config: AgentConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            client: None,
            user_id: None,
            mention_handlers: Arc::new(Mutex::new(Vec::new())),
            message_handlers: Arc::new(Mutex::new(Vec::new())),
            task_handlers: Arc::new(Mutex::new(Vec::new())),
            task_manager: Arc::new(Mutex::new(TaskManager::new())),
            shutdown_tx: None,
        })
    }

    /// Connect to the Matrix homeserver: log in and perform an initial sync.
    pub async fn connect(&mut self) -> Result<()> {
        info!(
            homeserver = %self.config.homeserver_url,
            username = %self.config.username,
            "Connecting to Matrix homeserver"
        );

        let homeserver = url::Url::parse(&self.config.homeserver_url)?;
        let client = Client::builder()
            .homeserver_url(homeserver)
            .build()
            .await
            .map_err(|e| ConstellationError::Connection(format!("failed to build client: {e}")))?;

        // Log in with username/password.
        let mut login = client
            .matrix_auth()
            .login_username(&self.config.username, &self.config.password);
        if let Some(ref device_id) = self.config.device_id {
            login = login.device_id(device_id);
        }
        login
            .initial_device_display_name(
                self.config
                    .display_name
                    .as_deref()
                    .unwrap_or(&self.config.username),
            )
            .send()
            .await?;

        info!("Logged in successfully");

        // Set display name if provided.
        if let Some(ref display_name) = self.config.display_name {
            if let Err(e) = client.account().set_display_name(Some(display_name)).await {
                warn!("Failed to set display name: {e}");
            }
        }

        // Perform initial sync to get room state.
        client.sync_once(SyncSettings::default()).await?;
        info!("Initial sync complete");

        self.user_id = client.user_id().map(|id| id.to_owned());
        self.client = Some(client.clone());

        // Auto-join configured rooms.
        for room_alias in &self.config.auto_join_rooms {
            match self.join_room_inner(&client, room_alias).await {
                Ok(handle) => {
                    info!(room = %room_alias, room_id = %handle.room_id(), "Auto-joined room")
                }
                Err(e) => warn!(room = %room_alias, error = %e, "Failed to auto-join room"),
            }
        }

        Ok(())
    }

    /// Gracefully disconnect from the homeserver.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }
        if let Some(ref client) = self.client {
            info!("Logging out");
            client
                .matrix_auth()
                .logout()
                .await
                .map_err(|e| ConstellationError::Connection(format!("logout failed: {e}")))?;
        }
        self.client = None;
        self.user_id = None;
        Ok(())
    }

    /// Join a room by alias (e.g. `#agents:constellation.local`) or room ID.
    pub async fn join_room(&self, room_alias: &str) -> Result<RoomHandle> {
        let client = self.require_client()?;
        self.join_room_inner(client, room_alias).await
    }

    async fn join_room_inner(&self, client: &Client, room_alias: &str) -> Result<RoomHandle> {
        info!(room = %room_alias, "Joining room");

        let response = client
            .join_room_by_id_or_alias(
                <&RoomAliasId>::try_from(room_alias)?.into(),
                &[],
            )
            .await?;

        let room = client
            .get_room(response.room_id())
            .ok_or_else(|| {
                ConstellationError::Room(format!(
                    "room {} joined but not found in client state",
                    response.room_id()
                ))
            })?;

        Ok(RoomHandle::new(room))
    }

    /// Create a new room and optionally invite agents.
    pub async fn create_room(
        &self,
        name: &str,
        invited_agents: &[&str],
    ) -> Result<RoomHandle> {
        let client = self.require_client()?;
        info!(name = %name, "Creating room");

        let mut request = CreateRoomRequest::new();
        request.name = Some(name.to_string());

        let invite: std::result::Result<Vec<OwnedUserId>, _> = invited_agents
            .iter()
            .map(|a| OwnedUserId::try_from(*a))
            .collect();
        request.invite = invite?;

        let response = client.create_room(request).await?;
        let room = client
            .get_room(response.room_id())
            .ok_or_else(|| {
                ConstellationError::Room(format!(
                    "room {} created but not found in client state",
                    response.room_id()
                ))
            })?;

        Ok(RoomHandle::new(room))
    }

    /// Send a message to a room.
    pub async fn send_message(&self, room: &RoomHandle, msg: Message) -> Result<()> {
        room.send_message(&msg).await
    }

    /// Send a message that @-mentions a specific agent.
    pub async fn mention_agent(
        &self,
        room: &RoomHandle,
        agent_user_id: &str,
        msg: Message,
    ) -> Result<()> {
        // Use the localpart as display name if we can't look it up.
        let display_name = agent_user_id.split(':').next().unwrap_or(agent_user_id);
        room.send_mention(agent_user_id, display_name, &msg).await
    }

    /// Register a handler that fires when this agent is @-mentioned.
    pub fn on_mention(&self, handler: impl Fn(MentionEvent) + Send + Sync + 'static) {
        let handlers = self.mention_handlers.clone();
        tokio::spawn(async move {
            handlers.lock().await.push(Arc::new(handler));
        });
    }

    /// Register a handler for all incoming messages.
    pub fn on_message(&self, handler: impl Fn(MessageEvent) + Send + Sync + 'static) {
        let handlers = self.message_handlers.clone();
        tokio::spawn(async move {
            handlers.lock().await.push(Arc::new(handler));
        });
    }

    /// Register a handler for structured task events.
    pub fn on_task(&self, handler: impl Fn(TaskEvent) + Send + Sync + 'static) {
        let handlers = self.task_handlers.clone();
        tokio::spawn(async move {
            handlers.lock().await.push(Arc::new(handler));
        });
    }

    /// Create a task, send it to a room, and track it.
    pub async fn create_task(&self, room: &RoomHandle, task: Task) -> Result<String> {
        let task_id = task.id.clone();
        info!(task_id = %task_id, task_type = %task.task_type, "Creating task");

        // Track the task locally.
        {
            let mut mgr = self.task_manager.lock().await;
            mgr.create(
                &task_id,
                &task.task_type,
                task.payload.clone(),
                room.room_id(),
            );
        }

        // Send the task as a message with constellation metadata.
        let msg = Message::text(format!("[task:{}] {}", task.task_type, task.id))
            .with_metadata(task.to_metadata());
        room.send_message(&msg).await?;

        Ok(task_id)
    }

    /// Mark a task as completed and send the result to its originating room.
    pub async fn complete_task(&self, task_id: &str, result: TaskResult) -> Result<()> {
        let room_id = {
            let mut mgr = self.task_manager.lock().await;
            let record = mgr
                .get(task_id)
                .ok_or_else(|| ConstellationError::Task(format!("task not found: {task_id}")))?;
            let room_id = record.room_id.clone();
            mgr.complete(task_id, result.clone())?;
            room_id
        };

        // Send completion message back to the room.
        let client = self.require_client()?;
        if let Some(room) = client.get_room(
            <&matrix_sdk::ruma::RoomId>::try_from(room_id.as_str())?,
        ) {
            let handle = RoomHandle::new(room);
            let status_str = match result.status {
                crate::message::TaskStatus::Completed => "completed",
                crate::message::TaskStatus::Failed => "failed",
                _ => "updated",
            };
            let msg = Message::text(format!("[task-result:{status_str}] {task_id}"));
            handle.send_message(&msg).await?;
        }

        Ok(())
    }

    /// Access the task manager.
    pub async fn task_manager(&self) -> tokio::sync::MutexGuard<'_, TaskManager> {
        self.task_manager.lock().await
    }

    /// Start the sync loop, dispatching incoming events to registered handlers.
    ///
    /// This blocks until [`disconnect`] is called or the process is interrupted.
    pub async fn run(&mut self) -> Result<()> {
        let client = self.require_client()?.clone();
        let my_user_id = self
            .user_id
            .clone()
            .ok_or_else(|| ConstellationError::Connection("not logged in".to_string()))?;

        let mention_handlers = self.mention_handlers.clone();
        let message_handlers = self.message_handlers.clone();
        let task_handlers = self.task_handlers.clone();

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        self.shutdown_tx = Some(shutdown_tx);

        info!("Starting sync loop");

        // Register the event handler on the client.
        client.add_event_handler({
            let my_user_id = my_user_id.clone();
            let mention_handlers = mention_handlers.clone();
            let message_handlers = message_handlers.clone();
            let task_handlers = task_handlers.clone();

            move |event: OriginalSyncRoomMessageEvent, room: Room| {
                let my_user_id = my_user_id.clone();
                let mention_handlers = mention_handlers.clone();
                let message_handlers = message_handlers.clone();
                let task_handlers = task_handlers.clone();

                async move {
                    // Ignore our own messages.
                    if event.sender == my_user_id {
                        return;
                    }

                    let room_id = room.room_id().to_string();
                    let sender = event.sender.to_string();

                    let body = match &event.content.msgtype {
                        MessageType::Text(text) => text.body.clone(),
                        _ => return,
                    };

                    // Extract constellation metadata directly from event content.
                    // We serialize just the content to access custom fields.
                    let content_raw = serde_json::to_value(&event.content).unwrap_or_default();
                    let metadata: Option<ConstellationMetadata> = content_raw
                        .get("ai.constellation.metadata")
                        .and_then(|m| serde_json::from_value(m.clone()).ok());

                    // Build raw event representation for MessageEvent.
                    let raw_event = serde_json::json!({
                        "sender": sender,
                        "room_id": room_id,
                        "content": content_raw,
                        "event_id": event.event_id.to_string(),
                        "origin_server_ts": event.origin_server_ts.get(),
                    });

                    // --- Dispatch to message handlers ---
                    {
                        let handlers = message_handlers.lock().await;
                        let msg_event = MessageEvent {
                            sender: sender.clone(),
                            room_id: room_id.clone(),
                            body: body.clone(),
                            raw_event: raw_event.clone(),
                        };
                        for handler in handlers.iter() {
                            handler(msg_event.clone());
                        }
                    }

                    // --- Dispatch to mention handlers if this agent is mentioned ---
                    let mentions = parse_mentions(&body);
                    let my_id_str = my_user_id.to_string();
                    if mentions.iter().any(|m| m == &my_id_str) {
                        let handlers = mention_handlers.lock().await;
                        let mention_event = MentionEvent {
                            sender: sender.clone(),
                            room_id: room_id.clone(),
                            body: body.clone(),
                            metadata: metadata.clone(),
                            mentioned_agents: mentions.clone(),
                        };
                        for handler in handlers.iter() {
                            handler(mention_event.clone());
                        }
                    }

                    // --- Dispatch to task handlers if constellation metadata is present ---
                    if let Some(ref meta) = metadata {
                        let handlers = task_handlers.lock().await;
                        let task_event = TaskEvent {
                            sender: sender.clone(),
                            room_id: room_id.clone(),
                            task_id: meta.task_id.clone(),
                            task_type: meta.task_type.clone(),
                            payload: meta.payload.clone(),
                            priority: meta.priority,
                        };
                        for handler in handlers.iter() {
                            handler(task_event.clone());
                        }
                    }
                }
            }
        });

        // Run the sync loop until shutdown.
        let sync_settings = SyncSettings::default();
        tokio::select! {
            _ = async {
                loop {
                    match client.sync_once(sync_settings.clone()).await {
                        Ok(response) => {
                            debug!("Sync tick: {} joined rooms", response.rooms.join.len());
                        }
                        Err(e) => {
                            error!("Sync error: {e}");
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        }
                    }
                }
            } => {}
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received, stopping sync loop");
            }
        }

        Ok(())
    }

    /// Get a reference to the underlying Matrix client.
    pub fn client(&self) -> Option<&Client> {
        self.client.as_ref()
    }

    /// Get this agent's user ID (available after connect).
    pub fn user_id(&self) -> Option<&OwnedUserId> {
        self.user_id.as_ref()
    }

    fn require_client(&self) -> Result<&Client> {
        self.client
            .as_ref()
            .ok_or_else(|| ConstellationError::Connection("not connected".to_string()))
    }
}
