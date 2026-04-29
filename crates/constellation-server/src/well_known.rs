use axum::{extract::State, Json};
use constellation_a2a::AgentCard;

use crate::state::AppState;

pub async fn get_agent_card(State(state): State<AppState>) -> Json<AgentCard> {
    Json(state.card.clone())
}
