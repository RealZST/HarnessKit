use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::state::WebState;

/// Identity of the node serving this UI. Lets the frontend distinguish multiple
/// browser tabs that each point at a different remote `hk serve` instance.
#[derive(Serialize)]
pub struct ServerInfo {
    pub node_name: String,
}

pub async fn server_info(State(state): State<WebState>) -> Json<ServerInfo> {
    Json(ServerInfo {
        node_name: state.node_name.clone(),
    })
}
