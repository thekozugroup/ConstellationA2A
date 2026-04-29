//! Shared application state threaded through Axum handlers.

use constellation_a2a::AgentCard;
use constellation_store::Store;
use std::sync::Arc;

/// Axum application state shared across all request handlers.
#[derive(Clone)]
pub struct AppState {
    /// Shared SQLite store for tasks and peers.
    pub store: Arc<Store>,
    /// This agent's own card, served at `/.well-known/agent.json`.
    pub card: AgentCard,
}
