use axum::{
    response::IntoResponse,
    extract::{
        State,
        ws::{
            Message,
            WebSocket,
            WebSocketUpgrade,
        },
    },
};
use futures::SinkExt;
use crate::{
    app::AppState,
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
    user: AuthenticatedUser,
) {
    // Get or create the hub scoped strictly to this user
    let hub = state.subscribe_user(&user.user_id);
    let mut rx = hub.global_tx.subscribe();
    let mut shutdown_rx = state.shutdown_tx.subscribe();

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.recv() => {
                let _ = socket.close().await;
                break;
            }

            // Utf8Bytes directly for this user
            res = rx.changed() => {
                if res.is_err() {
                    break;
                }

                // Zero-copy pointer clone
                let payload = rx.borrow_and_update().clone();
                if !payload.is_empty() && socket.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
        }
    }

    // Remove if this user has no other active WebSockets
    state.cleanup_user(&user.user_id, &hub);
}

