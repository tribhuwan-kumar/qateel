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
    aria2::types::DlWsMessage,
    auth::types::User as AuthenticatedUser,
};

/// Authenticated
pub async fn dl_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| dl(socket, state, user))
}

async fn dl(
    mut socket: WebSocket,
    state: AppState,
    user: AuthenticatedUser
) {
    let mut rx = state.history_tx.subscribe();
    loop {
        if rx.changed().await.is_err() {
            break;
        }

        let msg = rx.borrow_and_update().clone();

        match msg {
            DlWsMessage::Tick { user_id, tasks } => {
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
            DlWsMessage::Event { user_id, data } => {
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

