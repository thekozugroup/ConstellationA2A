use matrix_sdk::Room;
use matrix_sdk::ruma::events::room::message::RoomMessageEventContent;
use tracing::{debug, info};

use crate::error::{ConstellationError, Result};
use crate::message::{format_mention_message, ConstellationMetadata, Message};

/// A handle to a joined Matrix room, providing high-level messaging operations.
#[derive(Debug, Clone)]
pub struct RoomHandle {
    inner: Room,
}

impl RoomHandle {
    pub(crate) fn new(room: Room) -> Self {
        Self { inner: room }
    }

    /// The room ID as a string.
    pub fn room_id(&self) -> String {
        self.inner.room_id().to_string()
    }

    /// Get the display name of the room, if set.
    pub async fn display_name(&self) -> Result<Option<String>> {
        let name = self
            .inner
            .display_name()
            .await
            .map_err(|e| ConstellationError::Room(format!("failed to get room name: {e}")))?;
        Ok(Some(name.to_string()))
    }

    /// Get the list of joined member user IDs.
    pub async fn get_members(&self) -> Result<Vec<String>> {
        let members = self
            .inner
            .members(matrix_sdk::RoomMemberships::JOIN)
            .await
            .map_err(|e| ConstellationError::Room(format!("failed to get members: {e}")))?;
        Ok(members.iter().map(|m| m.user_id().to_string()).collect())
    }

    /// Send a plain text message to this room.
    pub async fn send_message(&self, msg: &Message) -> Result<()> {
        let content = if let Some(ref meta) = msg.metadata {
            self.build_content_with_metadata(&msg.body, None, meta)?
        } else {
            RoomMessageEventContent::text_plain(&msg.body)
        };

        debug!(room_id = %self.room_id(), "Sending message");
        self.inner
            .send(content)
            .await
            .map_err(|e| ConstellationError::Message(format!("failed to send message: {e}")))?;
        Ok(())
    }

    /// Send a message that @-mentions a specific user.
    pub async fn send_mention(
        &self,
        user_id: &str,
        display_name: &str,
        msg: &Message,
    ) -> Result<()> {
        let (plain, html) = format_mention_message(user_id, display_name, &msg.body);

        let content = if let Some(ref meta) = msg.metadata {
            self.build_content_with_metadata(&plain, Some(&html), meta)?
        } else {
            RoomMessageEventContent::text_html(plain, html)
        };

        info!(room_id = %self.room_id(), target = %user_id, "Sending mention");
        self.inner
            .send(content)
            .await
            .map_err(|e| ConstellationError::Message(format!("failed to send mention: {e}")))?;
        Ok(())
    }

    /// Build a `RoomMessageEventContent` with constellation metadata injected.
    fn build_content_with_metadata(
        &self,
        plain: &str,
        html: Option<&str>,
        metadata: &ConstellationMetadata,
    ) -> Result<RoomMessageEventContent> {
        let content = match html {
            Some(h) => RoomMessageEventContent::text_html(plain, h),
            None => RoomMessageEventContent::text_plain(plain),
        };

        // Serialize the content to JSON, inject metadata, deserialize back.
        let mut raw = serde_json::to_value(&content)?;
        if let Some(obj) = raw.as_object_mut() {
            obj.insert(
                "ai.constellation.metadata".to_string(),
                serde_json::to_value(metadata)?,
            );
        }
        let content = serde_json::from_value(raw)?;
        Ok(content)
    }

    /// Access the underlying matrix-sdk Room.
    pub fn inner(&self) -> &Room {
        &self.inner
    }
}
