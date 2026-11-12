use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::watch;
use sqlx::{Pool, Sqlite};
use tokio::sync::broadcast;
use axum::extract::ws::Utf8Bytes;
use crate::his::buf::{ProgressBuffer, ActiveTasks};

use crate::{
	user::UserHub,
	config::AppConf,
    status::SysStatus,
	aria2::types::Aria2Client,
	aria2::daemon::Aria2Daemon,
    aria2::types::{DlWsMessage, GlobalStatWsMessage},
};

#[derive(Clone)]
pub struct AppState {
	pub config: Arc<AppConf>,
    pub jwt_secret: Arc<String>,
    pub db: Pool<Sqlite>,
    pub aria2: Arc<Aria2Client>,
	pub daemon: Aria2Daemon,
	pub active_tasks: ActiveTasks,
	pub progress_buf: ProgressBuffer,
	pub shutdown_tx: Arc<broadcast::Sender<()>>,
    pub status_tx: Arc<watch::Sender<SysStatus>>,
    // pub history_tx: Arc<watch::Sender<DlWsMessage>>,
    // pub global_tx: Arc<watch::Sender<GlobalStatWsMessage>>,
	pub user_hubs: Arc<DashMap<String, Arc<UserHub>>>,
}

impl AppState {
	pub fn subscribe_user(&self, user_id: &str) -> Arc<UserHub> {
		self.user_hubs
			.entry(user_id.to_string())
			.or_insert_with(|| Arc::new(UserHub::new()))
			.clone()
	}

	pub fn cleanup_user(&self, user_id: &str, hub: &Arc<UserHub>) {
		if hub.tick_tx.receiver_count() == 0
			&& hub.event_tx.receiver_count() == 0
				&& hub.global_tx.receiver_count() == 0
		{
			self.user_hubs.remove_if(user_id, |_, h| Arc::ptr_eq(h, hub));
		}
	}

	/// Aria2 poller & event listener
	pub fn emit_dl(&self, msg: &DlWsMessage) {
		let user_id = match msg {
			DlWsMessage::Tick { user_id, .. } => user_id,
			DlWsMessage::Event { user_id, .. } => user_id,
		};

		// if user has no open WebSocket tab
		let Some(hub) = self.user_hubs.get(user_id) else {
			return;
		};

		if let Ok(json) = serde_json::to_string(msg) {
			let payload = Utf8Bytes::from(json);
			match msg {
				DlWsMessage::Tick { .. } => {
					let _ = hub.tick_tx.send(payload);
				}
				DlWsMessage::Event { .. } => {
					let _ = hub.event_tx.send(payload);
				}
			}
		}
	}

	pub fn emit_global(&self, msg: &GlobalStatWsMessage) {
		let GlobalStatWsMessage::Global { user_id, .. } = msg;

		let Some(hub) = self.user_hubs.get(user_id) else {
			return;
		};

		if let Ok(json) = serde_json::to_string(msg) {
			let _ = hub.global_tx.send(Utf8Bytes::from(json));
		}
	}
}

