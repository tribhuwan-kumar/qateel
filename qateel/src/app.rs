use std::sync::Arc;
use tokio::sync::watch;
use tokio::sync::broadcast;
use sqlx::{Pool, Sqlite};

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
    pub status_tx: Arc<watch::Sender<SysStatus>>,
    pub history_tx: Arc<broadcast::Sender<DdlWsMessage>>,
    pub global_tx: Arc<broadcast::Sender<GlobalStatWsMessage>>,
}
