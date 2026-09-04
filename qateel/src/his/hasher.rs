use tracing::debug;
use crate::AppState;

/// Calculates blake3 hash in a background blocking thread, kinda useless
pub fn spawn_content_hasher(
    state: AppState,
    gid: String,
    files_json: Option<String>
    ) {
    if let Some(json_str) = files_json {
        if let Ok(paths) = serde_json::from_str::<Vec<String>>(&json_str) {
            if let Some(main_file) = paths.first().cloned() {
                tokio::spawn(async move {
                    let hash_result = tokio::task::spawn_blocking(move || {
                        use std::io::Read;
                        let mut file = std::fs::File::open(&main_file).ok()?;
                        let mut hasher = blake3::Hasher::new();
                        let mut buffer = [0; 65536]; /* 64KB chunks */

                        while let Ok(count) = file.read(&mut buffer) {
                            if count == 0 { break; }
                            hasher.update(&buffer[..count]);
                        }
                        Some(hasher.finalize().to_hex().to_string())
                    }).await.unwrap_or(None);

                    if let Some(hash) = hash_result {
                        let _ = sqlx::query!(
                            "UPDATE download_history SET content_hash = ? WHERE gid = ?",
                            hash, gid
                        ).execute(&state.db).await;

                        debug!("Successfully hashed GID {:?}: {:?}", gid, hash);
                    }
                });
            }
        }
    }
}
