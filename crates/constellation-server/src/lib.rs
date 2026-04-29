//! Axum router for the A2A server.

pub mod rpc;
pub mod state;
pub mod well_known;

use axum::{
    routing::{get, post},
    Router,
};
pub use state::AppState;

/// Build the Axum router with all A2A routes wired up.
pub fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/", post(rpc::dispatch))
        .route("/.well-known/agent.json", get(well_known::get_agent_card))
        .with_state(state)
}

use anyhow::Result;
use tokio::net::TcpListener;

/// Start serving on `listener` with the given application state.
pub async fn run(state: AppState, listener: TcpListener) -> Result<()> {
    let app = build_app(state);
    axum::serve(listener, app).await?;
    Ok(())
}
