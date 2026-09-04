use serde::Serialize;
use axum::{
    response::IntoResponse,
    extract::{ State,
        ws::{
            Message,
            WebSocket,
            WebSocketUpgrade,
        },
    },
};

use crate::{
    app::AppState,
};

/// Qateel status
#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SysStatus {
    pub version: String,
    pub admin_exists: bool,
    pub aria2_alive: bool,
}

/// Unauthenticated webSocket
pub async fn status_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| sys_status(socket, state))
}

pub async fn sys_status(
    mut socket: WebSocket,
    state: AppState,
) {
    let mut rx = state.status_tx.subscribe();

    let init = {
        let current = rx.borrow();
        serde_json::to_string(&*current).unwrap()
    };

    if socket.send(Message::Text(init.into())).await.is_err() {
        return;
    }

    while rx.changed().await.is_ok() {
        let status = rx.borrow_and_update().clone();
        let json = serde_json::to_string(&status).unwrap();
        if socket.send(Message::Text(json.into())).await.is_err() {
            break;
        }
    }
}
