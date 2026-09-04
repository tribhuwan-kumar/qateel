use axum::{
    response::IntoResponse,
    extract::{State,
        ws::{
            Message,
            WebSocket,
            WebSocketUpgrade,
        },
    },
};

use crate::{
    app::AppState,
    aria2::types::GlobalStatWsMessage,
    auth::types::User as AuthenticatedUser,
};

/// Authenticated
pub async fn global_stat_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| global_stat(socket, state, user))
}

async fn global_stat(
    mut socket: WebSocket,
    state: AppState,
    user: AuthenticatedUser
) {
    let mut rx = state.global_tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        match &msg {
            GlobalStatWsMessage::Global { user_id, .. } => {
                if *user_id == user.user_id {
                    let text = serde_json::to_string(&msg).unwrap_or_default();
                    if socket.send(
                        Message::Text(text.into())
                    ).await.is_err() { break; }
                }
            },
        }
    }
}

