use std::path::Path;
use std::time::Duration;
use notify_debouncer_full::notify::event::{ModifyKind, RenameMode};
use notify_debouncer_full::{new_debouncer, notify::{RecursiveMode, EventKind}};

#[tokio::main]
async fn main() {
    let path_to_watch = Path::new("/home/trash/Downloads/aria2");

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let mut debouncer = new_debouncer(Duration::from_millis(500), None, move |res| {
        if let Err(e) = tx.blocking_send(res) {
            eprintln!("Failed to send event via channel: {:?}", e);
        }
    })
    .expect("Failed to create debouncer");

    // Start watching
    // debouncer.watch(path_to_watch, RecursiveMode::Recursive)

    println!("Watching directory: {:?}", path_to_watch);
	debouncer.watch(path_to_watch, RecursiveMode::NonRecursive);

    // Asynchronously consume detailed events
    while let Some(res) = rx.recv().await {
		match res {
			Ok(debounced_events) => {
				for debounced_event in debounced_events {
					let inner_event = debounced_event.event;

					match inner_event.kind {
						EventKind::Remove(_) => {
							for path in inner_event.paths {
								if let Some(file_name) = path.file_name() {
									println!("[DELETED] {:?}", file_name);
								}
							}
						}

						EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
							if inner_event.paths.len() >= 2 {
								let old_path = &inner_event.paths[0];
								let new_path = &inner_event.paths[1];

								if let (Some(old_name), Some(new_name)) = (old_path.file_name(), new_path.file_name()) {
									println!("[MOVED] {:?} -> {:?}", old_name, new_name);
								}
							}
						}

						_ => {} // Ignore other events like content modifications or creation
					}
				}
			}
			Err(errors) => {
				for error in errors {
					eprintln!("Watcher error: {:?}", error);
				}
			}
		}
	}
}
















use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

pub async fn add_uris(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<AddUriReq>,
) -> impl IntoResponse {
    let mut calls: Vec<serde_json::Value> = Vec::new();
    let mut generated_gids: Vec<String> = Vec::new();
    let mut call_indices: Vec<usize> = Vec::new();
    let mut results: Vec<serde_json::Value> = Vec::new();

    for (i, task) in payload.tasks.into_iter().enumerate() {
        // Rely entirely on frontend-provided options for duplicate resolution!
        let mut opt = task.options.unwrap_or_default();
        let info_hash = Extraction::extract_mgn_info_hash(&task.uri);
        let is_magnet = info_hash.is_some();

        let kind = match url::Url::parse(&task.uri) {
            Ok(u) => match u.scheme().to_ascii_lowercase().as_str() {
                "magnet" => Kind::Torrent,
                "http" | "https" | "ftp" | "sftp" => Kind::Uri,
                _ => Kind::Unknown,
            },
            Err(_) => Kind::Unknown,
        };

        if kind == Kind::Unknown {
            results.push(json!({ "index": i, "code": "INVALID_URI", "error": "Unsupported URL format" }));
            continue;
        }

        let gid = utils::generate_gid();
        opt.insert("gid".to_string(), json!(gid));

        if !is_magnet && !task.name.is_empty() {
            opt.insert("out".to_string(), json!(task.name.clone()));
        }

        // Database insertion (Wait state, tracked seamlessly)
        if let Err(e) = sqlx::query!(
            r#"
                INSERT INTO download_history
                (gid, user_id, kind, status, name, source_uri, total_length, is_resume_supported)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            gid, user.user_id, kind, "waiting", task.name, task.uri, task.total_length, task.is_resume_supported
        ).execute(&state.db).await {
            error!("Failed to register record for GID {:?}: {:?}", gid, e);
        }

        if is_magnet {
            let hash_str = info_hash.clone().unwrap().to_lowercase();
            let torrent_file_name = format!("{}-{}.torrent", user.user_id, hash_str);
            let torrent_path = state.config.data_dir.join(&torrent_file_name);

            // Directly inject the .torrent file downloaded during probe_uris_info
            if torrent_path.exists() {
                if let Ok(file_contents) = std::fs::read(&torrent_path) {
                    let b64 = BASE64.encode(&file_contents);
                    calls.push(json!({
                        "methodName": "aria2.addTorrent",
                        "params": [b64, [], opt]
                    }));
                    call_indices.push(i);
                    generated_gids.push(gid.clone());
                    continue;
                }
            }
        }

        // Standard addUri (or Magnet fallback if user skipped probe)
        calls.push(json!({
            "methodName": "aria2.addUri",
            "params": [[task.uri], opt]
        }));
        call_indices.push(i);
        generated_gids.push(gid.clone());
    }

    if calls.is_empty() {
        return (StatusCode::OK, Json(json!({ "results": results })));
    }

    // Execute multicall for standard URIs or cached Torrents
    match state.aria2.call("system.multicall", vec![json!(calls)]).await {
        Ok(gids_resp) => {
            if let Some(arr) = gids_resp.as_array() {
                for (arr_idx, res) in arr.iter().enumerate() {
                    let original_index = call_indices[arr_idx];
                    let expected_gid = &generated_gids[arr_idx];

                    if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {
                        let state_clone = state.clone();
                        let gid_clone = returned_gid.to_string();

                        tokio::spawn(async move {
                            if let Ok(opts_val) = state_clone.aria2.call("aria2.getOption", vec![json!(gid_clone)]).await {
                                let opts_str = serde_json::to_string(&opts_val).unwrap_or_default();
                                let _ = sqlx::query!("UPDATE download_history SET options = ? WHERE gid = ?", opts_str, gid_clone).execute(&state_clone.db).await;
                            }
                            HistoryService::refresh_gid(&state_clone, gid_clone).await;
                        });

                        results.push(json!({ "index": original_index, "gid": returned_gid }));
                    } else if let Some(err_obj) = res.as_object() {
                        let code_val = err_obj.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
                        let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        let _ = sqlx::query!("DELETE FROM download_history WHERE gid = ?", expected_gid).execute(&state.db).await;
                        results.push(json!({ "index": original_index, "code": code_val, "error": msg }));
                    }
                }
            }
            results.sort_by_key(|v| v.get("index").and_then(|i| i.as_u64()).unwrap_or(0));
            (StatusCode::OK, Json(json!({ "results": results })))
        },
        Err(e) => {
            for gid in generated_gids {
                let _ = sqlx::query!("DELETE FROM download_history WHERE gid = ?", gid).execute(&state.db).await;
            }
            (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() })))
        }
    }
}







































