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
    aria2::types::DdlWsMessage,
    auth::types::User as AuthenticatedUser,
};

/// Authenticated
pub async fn ddl_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ddl(socket, state, user))
}

async fn ddl(
    mut socket: WebSocket,
    state: AppState,
    user: AuthenticatedUser
) {
    let mut rx = state.history_tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        match msg {
            DdlWsMessage::Tick { user_id, tasks } => {
                if user_id == user.user_id {
                    let json = serde_json::json!({
                        "type": "tick",
                        "tasks": tasks
                    });
                    if socket.send(
                        Message::Text(json.to_string().into())
                    ).await.is_err() {
                        break;
                    }
                }
            },
            DdlWsMessage::Event { user_id, data } => {
                if user_id == user.user_id {
                    let json = serde_json::json!({
                        "type": "event",
                        "data": data
                    });
                    if socket.send(
                        Message::Text(json.to_string().into())
                    ).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}
