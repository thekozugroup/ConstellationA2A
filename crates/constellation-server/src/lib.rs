//! Axum router for the A2A server.

pub mod rpc;
pub mod state;
pub mod well_known;

use axum::{
    routing::{get, post},
    Router,
};
pub use state::AppState;

pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/", post(rpc::dispatch))
        .route("/.well-known/agent.json", get(well_known::get_agent_card))
        .with_state(state)
}
