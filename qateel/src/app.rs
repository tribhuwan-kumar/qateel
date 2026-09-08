use std::sync::Arc;
use tokio::sync::watch;
use sqlx::{Pool, Sqlite};
use tokio::sync::broadcast;
use crate::his::buf::{ProgressBuffer, ActiveTasks};

use crate::{
	config::AppConf,
    status::SysStatus,
	aria2::types::Aria2Client,
	aria2::daemon::Aria2Daemon,
    aria2::types::{DdlWsMessage, GlobalStatWsMessage},
};

#[derive(Clone)]
pub struct AppState {
	pub config: AppConf,
    pub jwt_secret: String,
    pub db: Pool<Sqlite>,
    pub aria2: Arc<Aria2Client>,
	pub daemon: Arc<Aria2Daemon>,
	pub active_tasks: ActiveTasks,
	pub progress_buffer: ProgressBuffer,
	pub shutdown_tx: broadcast::Sender<()>,
    pub status_tx: Arc<watch::Sender<SysStatus>>,
    pub history_tx: Arc<broadcast::Sender<DdlWsMessage>>,
    pub global_tx: Arc<broadcast::Sender<GlobalStatWsMessage>>,
}

