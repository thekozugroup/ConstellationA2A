use constellation_a2a::AgentCard;
use constellation_store::Store;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub card: AgentCard,
}
