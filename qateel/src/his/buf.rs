use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use crate::aria2::types::{
	TickTask, ItemMetaData
};

#[derive(Clone, Debug)]
pub struct ProgressBuffer {
    // Map of GID -> TickTask
    pub updates: Arc<RwLock<HashMap<String, TickTask>>>,
}

/// Specifically to update progress in db
impl ProgressBuffer {
    pub fn new() -> Self {
        Self {
            updates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_update(&self, update: TickTask) {
        let mut updates = self.updates.write().await;
        updates.insert(update.gid.clone(), update);
    }

	pub async fn remove(&self, gid: &str) {
        let mut updates = self.updates.write().await;
        updates.remove(gid);
    }

    pub async fn get_all_updates(&self) -> Vec<TickTask> {
        let mut updates = self.updates.write().await;
        updates.drain().map(|(_, v)| v).collect()
    }
}

#[derive(Clone, Debug)]
pub struct ActiveTasks {
    // Map of gid -> user_id
	pub tasks: Arc<RwLock<HashMap<String, ItemMetaData>>>,
}

/// Emit via WS only active tasks
impl ActiveTasks {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Insert or overwrite a task
    pub async fn insert(&self, gid: String, meta: ItemMetaData) {
        self.tasks.write().await.insert(gid, meta);
    }

    /// Remove a task and return it (used when task hits terminal state)
    pub async fn remove(&self, gid: &str) -> Option<ItemMetaData> {
        self.tasks.write().await.remove(gid)
    }

    /// Just get the user_id if needed elsewhere
	#[allow(unused)]
    pub async fn get_user_id(&self, gid: &str) -> Option<String> {
        self.tasks.read().await.get(gid).map(|meta| meta.user_id.clone())
    }

    /// Updates the active task with fresh telemetry
	/// Stuff i do to avoid heavy cloning :D
	/// No streaming tick for `is_trackable`
    pub async fn upsert_tick(
        &self,
        gid: &str,
		meta: ItemMetaData,
        upload_speed: String,
        download_speed: String,
        num_seeders: Option<String>,
		connections: Option<String>,
    ) -> Option<(String, TickTask)> {
        let mut tasks = self.tasks.write().await;
        if let Some(existing_meta) = tasks.get_mut(gid) {

			existing_meta.update_from_telemetry(meta);

            if existing_meta.is_terminal() || !existing_meta.is_streaming() {
                return None;
            }

            let tick_task = TickTask {
                gid: existing_meta.gid.clone(),
                name: existing_meta.name.clone(),
                kind: existing_meta.kind.clone(),
                status: existing_meta.status.clone(),
                dir: existing_meta.dir.clone(),
                source_uri: existing_meta.source_uri.clone(),
				is_resume_supported: existing_meta.is_resume_supported.clone(),
				files: existing_meta.files.clone(),
                upload_length: existing_meta.upload_length.clone().unwrap_or_else(|| "0".to_string()),
                completed_length: existing_meta.completed_length.clone().unwrap_or_else(|| "0".to_string()),
                total_length: existing_meta.total_length.clone().unwrap_or_else(|| "0".to_string()),
                verified_length: existing_meta.verified_length.clone(),
				connections,
				download_speed,
				upload_speed,
				num_seeders,
                seeder: existing_meta.seeder.clone(),
				verify_integrity_pending: existing_meta.verify_integrity_pending.clone(),
            };

            return Some((existing_meta.user_id.clone(), tick_task));
        }
        None
    }

	/// Updated telemetry status, returns `is_terminal`, `is_streaming`
    pub async fn tele_status(&self, gid: &str, meta: ItemMetaData) -> Option<(bool, bool)> {
        let mut tasks = self.tasks.write().await;
        if let Some(existing_meta) = tasks.get_mut(gid) {
            existing_meta.update_from_telemetry(meta);
            return Some((existing_meta.is_terminal(), existing_meta.is_streaming()));
        }
        None
    }
}

