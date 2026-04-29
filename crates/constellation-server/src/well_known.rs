//! Handler for the `/.well-known/agent.json` discovery endpoint.

use axum::{extract::State, Json};
use constellation_a2a::AgentCard;

use crate::state::AppState;

/// Return this agent's [`AgentCard`] as JSON.
pub async fn get_agent_card(State(state): State<AppState>) -> Json<AgentCard> {
    Json(state.card.clone())
}
