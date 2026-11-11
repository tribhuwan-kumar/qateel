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
    pub history_tx: Arc<watch::Sender<DlWsMessage>>,
    pub global_tx: Arc<watch::Sender<GlobalStatWsMessage>>,
}

