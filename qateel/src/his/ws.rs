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
use futures::SinkExt;
use tokio::sync::broadcast;
use crate::{
    app::AppState,
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
	let hub = state.subscribe_user(&user.user_id);
	let mut tick_rx = hub.tick_tx.subscribe();
	let mut event_rx = hub.event_tx.subscribe();
	let mut shutdown_rx = state.shutdown_tx.subscribe();

	loop {
		tokio::select! {
			biased;

			_ = shutdown_rx.recv() => {
				let _ = socket.close();
				break;
			}

			res = event_rx.recv() => {
				match res {
					Ok(payload) => {
						if socket.send(Message::Text(payload)).await.is_err() {
							break;
						}
					}
					Err(broadcast::error::RecvError::Lagged(_)) => {
						// Optionally send a "resync" message to ui to refetch list
						// Maybe later
					}
					Err(broadcast::error::RecvError::Closed) => break,
				}
			}

			res = tick_rx.changed() => {
				if res.is_err() {
					break;
				}
				let payload = tick_rx.borrow_and_update().clone();
				if !payload.is_empty() && socket.send(Message::Text(payload)).await.is_err() {
					break;
				}
			}
		}
	}

	state.cleanup_user(&user.user_id, &hub);
}

