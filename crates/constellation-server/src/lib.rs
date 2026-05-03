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

#[cfg(test)]
mod tests {
    use super::*;
    use constellation_a2a::{AgentCapabilities, AgentCard};
    use constellation_store::Store;
    use std::sync::Arc;
    use url::Url;

    #[tokio::test]
    async fn test_build_app() {
        let store = Store::open(":memory:").expect("Failed to open in-memory store");
        let card = AgentCard {
            name: "test-agent".to_string(),
            description: None,
            url: Url::parse("http://localhost:3000").unwrap(),
            version: "1.0.0".to_string(),
            capabilities: AgentCapabilities::default(),
            default_input_modes: vec!["text".to_string()],
            default_output_modes: vec!["text".to_string()],
            skills: vec![],
        };

        let state = AppState {
            store: Arc::new(store),
            card: card.clone(),
        };

        let app = build_app(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();

        let url = format!("http://{}/.well-known/agent.json", addr);
        let resp = client
            .get(&url)
            .send()
            .await
            .expect("Failed to get agent card");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        let resp_card: AgentCard = resp.json().await.expect("Failed to parse agent card");
        assert_eq!(resp_card, card);

        let rpc_url = format!("http://{}/", addr);
        let resp = client
            .post(&rpc_url)
            .send()
            .await
            .expect("Failed to POST to RPC");
        // We just verify it's not a 404. It might be 400 Bad Request or something else due to empty body.
        assert_ne!(resp.status(), reqwest::StatusCode::NOT_FOUND);
    }
}
