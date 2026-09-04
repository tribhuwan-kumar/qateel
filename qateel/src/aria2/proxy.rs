use axum::{
    extract::Json,
    http::StatusCode,
    extract::{State, Query},
    response::{IntoResponse},
};
use std::path::Path;
use tokio::process::Command;
use futures::future::join_all;
use serde_json::{json, Value};
use std::collections::HashMap;
use sqlx::{Sqlite, QueryBuilder};
use super::types::{
    Kind,
	Order,
	SortBy,
    GidReq,
    MoveReq,
    StopReq,
	UriMeta,
    PauseReq,
    RetryReq,
    GidStatus,
    ResumeReq,
    AddUriReq,
	UriInfoReq,
	UriInfoRes,
	SearchQuery,
    ItemMetaData,
    Aria2Options,
    RemoveHisReq,
    SetOptionReq,
	CheckNameReq,
	CheckNameRes,
    PaginationQuery,
	DuplicateAction,
    SetCmdOptionsReq,
    TorrentMagnetMeta,
    BatchAddTorrentReq,
    SetGlobalOptionReq,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use tracing::{info, debug, error, warn};
use crate::{
    utils,
    his::header,
    app::AppState,
    his::header::UNTITLED,
	his::extract::Extraction,
    his::service::HistoryService,
    auth::types::User as AuthenticatedUser,
};


/// Add all most all types of URIs
pub async fn add_uris(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<AddUriReq>,
) -> impl IntoResponse {
    let mut calls: Vec<Value> = Vec::new();
    let mut generated_gids: Vec<String> = Vec::new();
    let mut call_indices: Vec<usize> = Vec::new();
    let mut results: Vec<Value> = Vec::new();

    for (i, task) in payload.tasks.into_iter().enumerate() {
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
			results.push(serde_json::json!({
				"index": i,
				"code": "INVALID_URI",
				"error": "Unsupported or invalid URL format"
			}));
			continue;
		}

        // Apply aria2 deduplication options
        match task.duplicate_action {
            None => {
                opt.entry("auto-file-renaming".to_string()).or_insert(serde_json::json!("false"));
                opt.entry("allow-overwrite".to_string()).or_insert(serde_json::json!("false"));
            }
            Some(DuplicateAction::Copy) => {
                opt.insert("auto-file-renaming".to_string(), serde_json::json!("true"));
                opt.insert("allow-overwrite".to_string(), serde_json::json!("false"));
            }
            Some(DuplicateAction::Overwrite) => {
                opt.insert("auto-file-renaming".to_string(), serde_json::json!("false"));
                opt.insert("allow-overwrite".to_string(), serde_json::json!("true"));
            }
        }

        let gid = utils::generate_gid();
        opt.insert("gid".to_string(), serde_json::json!(gid));

        // Inject the pre-probed file name so aria2 uses it
        if !is_magnet {
            opt.insert("out".to_string(), serde_json::json!(task.name.clone()));
        }

        // Database insertion
        if let Err(e) = sqlx::query!(
            r#"
                INSERT INTO download_history
                (gid, user_id, kind, status, name, source_uri, total_length, is_resume_supported)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            gid,
            user.user_id,
            kind,
            "waiting",
            task.name,
            task.uri,
            task.total_length,
            task.is_resume_supported
        ).execute(&state.db).await {
            error!("Failed to register record for GID {:?}: {:?}", gid, e);
        }

        call_indices.push(i);
        generated_gids.push(gid.clone());

        calls.push(serde_json::json!({
            "methodName": "aria2.addUri",
            "params": [ [task.uri], opt ]
        }));
    }

    if calls.is_empty() {
        return (reqwest::StatusCode::OK, Json(serde_json::json!({ "results": results })));
    }

    // Execute multicall
    match state.aria2.call("system.multicall", vec![serde_json::json!(calls)]).await {
        Ok(gids_resp) => {
            if let Some(arr) = gids_resp.as_array() {
                for (arr_idx, res) in arr.iter().enumerate() {
                    let original_index = call_indices[arr_idx];
                    let expected_gid = &generated_gids[arr_idx];

                    if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {

                        let state_clone = state.clone();
                        let gid_clone = returned_gid.to_string();

                        tokio::spawn(async move {
                            if let Ok(opts_val) = state_clone.aria2.call(
                                    "aria2.getOption",
                                    vec![serde_json::json!(gid_clone)]
                                ).await {
                                let opts_str = serde_json::to_string(&opts_val).unwrap_or_default();
								let _ = sqlx::query!("
									UPDATE download_history
									SET options = ?
									WHERE gid = ?",
									opts_str,
									gid_clone
                                ).execute(&state_clone.db).await;
                            }

                            HistoryService::refresh_gid(&state_clone, gid_clone).await;
                        });

                        results.push(serde_json::json!({
                            "index": original_index,
                            "gid": returned_gid
                        }));
                    } else if let Some(err_obj) = res.as_object() {
                        let borrow_j = &json!(0);
                        let code_val = err_obj.get("code").unwrap_or(borrow_j);
                        let code_val = code_val.as_i64().unwrap_or(0);
                        let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");

                        let _ = sqlx::query!(
                            "DELETE FROM download_history
                            WHERE gid = ?",
                            expected_gid
                        ).execute(&state.db).await;

                        results.push(serde_json::json!({
                            "index": original_index,
                            "code": code_val,
                            "error": msg
                        }));
                    }
                }
            }
            results.sort_by_key(|v| v.get("index").and_then(|i| i.as_u64()).unwrap_or(0));
            (reqwest::StatusCode::OK, Json(serde_json::json!({ "results": results })))
        },
        Err(e) => {
            for gid in generated_gids {
                let _ = sqlx::query!("DELETE FROM download_history WHERE gid = ?", gid).execute(&state.db).await;
            }
            (reqwest::StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": e.to_string() })))
        }
    }
}

/// Get preflight info for all types of uris
pub async fn probe_uris_info(
    State(state): State<AppState>,
    Json(payload): Json<UriInfoReq>,
) -> impl IntoResponse {
    let mut tasks = Vec::new();
    let mut results = Vec::new();

    for uri in payload.uris {
        let uri_lower = uri.to_ascii_lowercase();
        let is_magnet = uri_lower.starts_with("magnet:");
        let is_ftp = uri_lower.starts_with("ftp://");
        let is_sftp = uri_lower.starts_with("sftp://");
        let is_http = uri_lower.starts_with("http://") || uri_lower.starts_with("https://");


        if !is_magnet && !is_ftp && !is_sftp && !is_http {
            results.push(UriInfoRes {
                uri: uri.clone(),
                name: uri.clone(),
                total_length: "0".to_string(),
                is_resume_supported: false,
                is_magnet: false,
                is_ftp: false,
                is_sftp: false,
                is_duplicate: false,
                existing_gid: None,
                conflict_type: None,
                conflict_msg: None,
                probe_error: Some("Unsupported or invalid URL format".to_string()),
            });
            continue; // Skip spawning a thread pool task for this item
        }

        let state_clone = state.clone();

        // Spawn each probe onto the tokio thread pool concurrently
        tasks.push(tokio::spawn(async move {
            let info_hash = Extraction::extract_mgn_info_hash(&uri);

            let mut meta = UriMeta {
                name: String::new(),
                total_length: "0".to_string(),
                resumable: false
            };
            let mut probe_error = None;

            // Skip probing entirely for magnet links
            if is_magnet {
                meta.name = Extraction::extract_mgn_name(&uri).unwrap_or_else(|| uri.clone());
                meta.resumable = true;
            } else if is_ftp || is_sftp {
                meta.name = Extraction::extract_url_filename(&uri).unwrap_or_else(|| uri.clone());
                meta.resumable = true;
            }
            else {
                // Only use the HTTP Prober for http/https links
                match header::uri_metadata(&uri).await {
                    Ok(fetched_meta) => meta = fetched_meta,
                    Err((err_msg, fallback_meta)) => {
                        probe_error = Some(err_msg);
                        meta = fallback_meta;
                    }
                }
            }

            // Check database, disk for duplicates
            let duplicate_check = find_duplicate(
                &state_clone,
                &uri,
                &meta.name,
                &meta.total_length,
                info_hash.as_deref()
            ).await;

            UriInfoRes {
                uri,
                name: meta.name,
                total_length: meta.total_length,
                is_resume_supported: meta.resumable,
                is_magnet,
                is_ftp,
                is_sftp,
                is_duplicate: duplicate_check.is_some(),
                existing_gid: duplicate_check.as_ref().and_then(|d| d.0.clone()),
                conflict_type: duplicate_check.as_ref().map(|d| d.2.clone()),
                conflict_msg: duplicate_check.as_ref().map(|d| d.3.clone()),
                probe_error: probe_error,
            }
        }));
    }

    // Wait for all http probes and db queries to finish all execution
    let resolved_tasks = join_all(tasks).await;

    for res in resolved_tasks {
        if let Ok(preflight_data) = res {
            results.push(preflight_data);
        }
    }

    let is_completely_offline = results.iter().any(|r| {
        r.probe_error.as_deref() == Some("No internet connection")
    });

    if is_completely_offline {
        for item in &mut results {
            if item.is_magnet || item.is_ftp || item.is_sftp {
                item.probe_error = Some("No internet connection".to_string());
            }
        }
    }

    (StatusCode::OK, Json(serde_json::json!({ "data": results })))
}

async fn find_duplicate(
    state: &AppState,
    uri: &str,
    name: &str,
    _total_length: &str,
    info_hash: Option<&str>
) -> Option<(Option<String>, String, String, String)> {     // (Option<Gid>, Name, Conflict type, Conflict message)

    // Check database first
    let records = sqlx::query!(
        r#"
            SELECT gid, name, files, dir, info_hash, source_uri
            FROM download_history
            WHERE (info_hash = $1 AND $1 IS NOT NULL)
               OR (source_uri = $2 AND $2 IS NOT NULL)
               OR (name = $3 AND $3 IS NOT NULL AND $3 != '' AND $3 != $4)
        "#,
        info_hash, uri, name, UNTITLED
    ).fetch_all(&state.db).await.unwrap_or_default();

    for rec in records {
        let mut files_exist = false;

        if let Some(files_str) = &rec.files {
            if let Ok(paths) = serde_json::from_str::<Vec<String>>(files_str) {
                files_exist = paths.iter().any(|p| Path::new(p).exists());
            }
        }

        if !files_exist {
            if let (Some(dir), Some(rec_name)) = (&rec.dir, &rec.name) {
                files_exist = Path::new(dir).join(rec_name).exists();
            }
        }

        let conflict_type = if info_hash.is_some() && rec.info_hash.as_deref() == info_hash {
            "hash".to_string()
        } else if rec.source_uri.as_deref() == Some(uri) {
            "uri".to_string()
        } else {
            "file".to_string()
        };

        let conflict_msg = if info_hash.is_some() && rec.info_hash.as_deref() == info_hash {
            "Torrent/magnet link already exists.".to_string()
        } else if rec.source_uri.as_deref() == Some(uri) {
            "Downloaded content from this URI already exists.".to_string()
        } else {
            "File already exists in the download dir.".to_string()
        };

        // Torrents always trigger.
        // URIs only trigger if the file is physically on disk.
        if info_hash.is_some() || files_exist {
            return Some((
                Some(rec.gid),
                rec.name.unwrap_or_else(|| "<Unknown>".to_string()),
                conflict_type,
                conflict_msg,
            ));
        }
    }

    // Check untracked physical files on disk
    // If the DB query found nothing,
    //  * explicitly check if a file with this name is sitting in the aria2 directory.
    if !name.is_empty() && name != UNTITLED {
        let physical_path = Path::new(&state.config.aria2_dir).join(name);
        if physical_path.exists() {
            return Some((
                None,                               // No GID because the database doesn't know about this file
                name.to_string(),
                "untrackedFile".to_string(),
                "An untracked file with this name already exists in the download directory.".to_string()
            ));
        }
    }

    None
}

/// Dynamic endpoint to check if a newly typed name conflicts with db or Disk
pub async fn check_name_conflict(
    State(state): State<AppState>,
    Json(payload): Json<CheckNameReq>,
) -> impl IntoResponse {
    let name = payload.name.trim();
    if name.is_empty() || name == UNTITLED {
        return (StatusCode::OK, Json(CheckNameRes { is_conflict: false, msg: None }));
    }

    let physical_path = Path::new(&state.config.aria2_dir).join(name);

    // Check database
    let db_exists = sqlx::query!(
        "SELECT gid FROM download_history WHERE name = ? LIMIT 1",
        name
    ).fetch_optional(&state.db).await.unwrap_or_default();

    if db_exists.is_some() && physical_path.exists()  {
        return (StatusCode::OK, Json(CheckNameRes {
            is_conflict: true,
            msg: Some("File with this name already exists in database & disk.".to_string())
        }));
    }

    // Check disk
    if physical_path.exists() {
        return (StatusCode::OK, Json(CheckNameRes {
            is_conflict: true,
            msg: Some("File with this name already exists on disk.".to_string())
        }));
    }

	(StatusCode::OK, Json(CheckNameRes { is_conflict: false, msg: None }))
}


/// Specially for adding torrent files
pub async fn add_torrents(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<BatchAddTorrentReq>,
) -> impl IntoResponse {
    if payload.torrents.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "No torrents provided" })));
    }
    /*
      * Build the multicall params
      * Transform list of inputs into a list of aria2 method calls
    */
    let mut calls: Vec<Value> = Vec::new();
    let mut generated_gids: Vec<String> = Vec::new();
	let mut saved_file_paths: Vec<std::path::PathBuf> = Vec::new();

    for item in payload.torrents.into_iter() {
        let gid = utils::generate_gid();
        let mut opt = item.options.unwrap_or_else(|| serde_json::json!({}));

        if let Some(map) = opt.as_object_mut() {
            map.insert("gid".to_string(), serde_json::json!(gid));
        }

		let torrent_bytes = match STANDARD.decode(&item.torrent) {
			Ok(bytes) => bytes,
			Err(e) => {
				error!("Failed to decode Base64 torrent string: {:?}", e);
				for path in saved_file_paths {
					let _ = tokio::fs::remove_file(path).await;
				}
				return (StatusCode::BAD_REQUEST, Json(json!(
							{ "error": format!("Invalid Base64 payload: {}", e) })));
			}
		};

		let file_name = format!("{}-{}.torrent", user.user_id, gid);
		let torrent_path = state.config.data_dir.join("torrents").join(&file_name);

		if let Some(parent_dir) = torrent_path.parent() {
			if let Err(e) = tokio::fs::create_dir_all(parent_dir).await {
				error!("Failed to create torrents directory: {:?}", e);
				for path in saved_file_paths {
					let _ = tokio::fs::remove_file(path).await;
				}
				return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(
							{ "error": "Failed to initialize server storage directories" })));
			}
		}

		if let Err(e) = tokio::fs::write(&torrent_path, torrent_bytes).await {
			error!("Failed to write torrent file to disk: {:?}", e);
			for path in saved_file_paths {
				let _ = tokio::fs::remove_file(path).await;
			}
			return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(
						{ "error": "Failed to persist torrent data to server storage" })));
		}

		saved_file_paths.push(torrent_path.clone());

        if let Err(e) = sqlx::query!(
            r#"
                INSERT INTO download_history (gid, user_id, kind, status)
                VALUES (?, ?, 'torrent', 'waiting')
            "#,
            gid,
            user.user_id
        ).execute(&state.db).await{
            error!("Failed to register record for GID {:?}: {:?}", gid, e);
			for path in saved_file_paths {
				let _ = tokio::fs::remove_file(path).await;
			}
			return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!(
						{ "error": "Database tracking registration failed" })));
		};

        generated_gids.push(gid.clone());

        calls.push(json!({
            "methodName": "aria2.addTorrent",
            "params": [
                item.torrent,
                [],                 /* Webseeding URIs, keeping it empty till now */
                opt
            ]
        }));
    }

    /*
      * Call system.multicall
      * Params for multicall is an array containing the array of calls: [ [call1, call2] ]
    */
    match state.aria2.call("system.multicall", vec![json!(calls)]).await {
        Ok(gids_resp) => {
            let mut results = Vec::new();

            if let Some(arr) = gids_resp.as_array() {
                for (i, res) in arr.iter().enumerate() {

                    let expected_gid = &generated_gids[i];
					let file_to_keep = &saved_file_paths[i];

                    if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {
                        let state_clone = state.clone();
                        let gid_clone = returned_gid.to_string();

                        tokio::spawn(async move {
                            if let Ok(opts_val) = state_clone.aria2.call("aria2.getOption", vec![json!(gid_clone)]).await {
                                let opts_str = serde_json::to_string(&opts_val).unwrap_or_default();
								let result = sqlx::query!(
									"UPDATE download_history SET options = ? WHERE gid = ?",
									opts_str,
									gid_clone
								)
								.execute(&state_clone.db)
								.await;

								if let Err(e) = result {
									error!("Failed to update options in database for GID {:?}: {:?}", gid_clone, e);
								}
                            }
                            HistoryService::refresh_gid(&state_clone, gid_clone).await;
                        });
                        results.push(json!({
                            "index": i,
                            "gid": returned_gid,
                        }));
                    }
                    else if let Some(err_obj) = res.as_object() {
                        let borrow_j = &json!(0);
                        let code = err_obj.get("code").unwrap_or(borrow_j);
                        let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");
                        let _ = sqlx::query!(
                            "DELETE FROM download_history WHERE gid = ?",
                            expected_gid
                        ).execute(&state.db).await;

						// Also delete its companion file from the filesystem
                        let _ = tokio::fs::remove_file(file_to_keep).await;

                        results.push(json!({
                            "index": i,
                            "code": code,
                            "error": msg,
                        }));
                    }
                    else {
                        results.push(json!({
							"index": i,
							"error": "Aria2 returned invalid response format"
						}));
                    }
                }
            }
            (StatusCode::OK, Json(json!({ "results": results })))
        }
        Err(e) => {
            error!("Failed to batch torrents: {}", e);
            for (i, gid) in generated_gids.iter().enumerate() {
                let _ = sqlx::query!(
                    "DELETE FROM download_history WHERE gid = ?",
                    gid
                ).execute(&state.db).await;
				let _ = tokio::fs::remove_file(&saved_file_paths[i]).await;
            }
            (StatusCode::BAD_GATEWAY, Json(json!({ "error": e })))
        }
    }
}


/// ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.pause
/// aria2.pause
/// aria2.pauseAll
/// aria2.forcePause
/// aria2.forcePauseAll
pub async fn pause(
    State(state): State<AppState>,
    Json(payload): Json<PauseReq>,
) -> impl IntoResponse {
    let is_force_all = payload.force_pause_all.unwrap_or(false);
    let is_all = payload.pause_all.unwrap_or(false);

    // `pauseAll` / `forcePauseAll` directly
	// No multicall needed for a single global command
    if is_force_all || is_all {
        let method = if is_force_all {
            "aria2.forcePauseAll"
        } else {
            "aria2.pauseAll"
        };

        return match state.aria2.call(method, vec![]).await {
            Ok(_) => {
                if let Some(ref gids) = payload.gids {
                    for gid in gids {
                        let state_clone = state.clone();
                        let gid_clone = gid.clone();
						// Mark to `resume` to `waiting`
                        tokio::spawn(async move {
                            HistoryService::handle_event(&state_clone, gid_clone, Some("paused")).await;
                        });
                    }
                }
                info!("Successfully executed `{}`", method);
                (
                    StatusCode::OK,
                    Json(json!({ "status": "paused", "method": method })),
                )
                    .into_response()
            }
            Err(e) => {
                error!("`{}` failed: {}", method, e);
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": e.to_string(), "success": false })),
                )
                    .into_response()
            }
        };
    }

    // Validate GIDs for the batch multicall
    let gids = match &payload.gids {
        Some(gids) if !gids.is_empty() => gids,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "At least one GID or a pause_all flag is required" })),
            )
                .into_response();
        }
    };

    let method = if payload.force_pause.unwrap_or(false) {
        "aria2.forcePause"
    } else {
        "aria2.pause"
    };

    // Build the multicall params
    let calls: Vec<Value> = gids
        .iter()
        .map(|gid| {
            json!({
                "methodName": method,
                "params": [gid]
            })
        })
        .collect();

    // Execute system.multicall
    match state.aria2.call("system.multicall", vec![json!(calls)]).await {
        Ok(multicall_resp) => {
            let mut succeeded = Vec::new();
            let mut failed = Vec::new();

            if let Some(arr) = multicall_resp.as_array() {
                for (i, res) in arr.iter().enumerate() {
                    let expected_gid = &gids[i];

                    // Multicall returns successes as a nested array: [[ "gid1" ], [ "gid2" ]]
                    if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {

                        let state_clone = state.clone();
                        let gid_clone = returned_gid.to_string();

                        // Fire and forget history update to avoid blocking response
						// Mark to `resume` to `waiting`
                        tokio::spawn(async move {
                            HistoryService::handle_event(&state_clone, gid_clone, Some("paused")).await;
                        });

                        succeeded.push(expected_gid.clone());
                    }
                    // Errors are returned as: { "code": ..., "message": ... }
                    else if let Some(err_obj) = res.as_object() {
                        let borrow_j = &json!(0);
                        let code = err_obj.get("code").unwrap_or(borrow_j);
                        let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");

                        failed.push(json!({
                            "gid": expected_gid,
                            "code": code,
                            "error": msg
                        }));
                    }
                    else {
                        failed.push(json!({
                            "gid": expected_gid,
                            "error": "Aria2 returned invalid response format"
                        }));
                    }
                }
            }

            info!(
                "Batch `{}` executed via multicall: {} succeeded, {} failed",
                method,
                succeeded.len(),
                failed.len()
            );

            // Build response based on outcomes
            if succeeded.is_empty() {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": "Failed to pause specified downloads",
                        "method": method,
                        "failed": failed
                    })),
                ).into_response();
            }

            (
                StatusCode::OK,
                Json(json!({
                    "status": "paused",
                    "method": method,
                    "succeeded": succeeded,
                    "failed": failed
                })),
            ).into_response()
        }
        Err(e) => {
            error!("`system.multicall` failed for {}: {}", method, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string(), "success": false })),
            ).into_response()
        }
    }
}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.unpause
/// `aria2.unpause`
/// `aria2.unpauseAll`
/// Paused to waiting
pub async fn resume(
    State(state): State<AppState>,
    Json(payload): Json<ResumeReq>,
) -> impl IntoResponse {
    let is_all = payload.resume_all.unwrap_or(false);

    // Handle `unpauseAll` directly
    if is_all {
        let method = "aria2.unpauseAll";

        return match state.aria2.call(method, vec![]).await {
            Ok(_) => {
                if let Some(ref gids) = payload.gid {
                    for gid in gids {
                        let state_clone = state.clone();
                        let gid_clone = gid.clone();
						// Mark to `resume` to `waiting`
                        tokio::spawn(async move {
                            HistoryService::handle_event(&state_clone, gid_clone, Some("waiting")).await;
                        });
                    }
                }
                info!("Successfully executed `{}`", method);
                (
                    StatusCode::OK,
                    Json(json!({ "status": "resumed", "method": method })),
                ).into_response()
            }
            Err(e) => {
                error!("`{}` failed: {}", method, e);
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": e.to_string(), "success": false })),
                ).into_response()
            }
        };
    }

    // Validate GIDs for the batch multicall
    let gids = match &payload.gid {
        Some(gids) if !gids.is_empty() => gids,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "At least one GID or a `resume_all` flag is required" })),
            ).into_response();
        }
    };

    let method = "aria2.unpause";

    // Build the multicall params
    let calls: Vec<Value> = gids
        .iter()
        .map(|gid| {
            json!({
                "methodName": method,
                "params": [gid]
            })
        })
        .collect();

    // Execute system.multicall
    match state.aria2.call("system.multicall", vec![json!(calls)]).await {
        Ok(multicall_resp) => {
            let mut succeeded = Vec::new();
            let mut failed = Vec::new();

            if let Some(arr) = multicall_resp.as_array() {
                for (i, res) in arr.iter().enumerate() {
                    let expected_gid = &gids[i];

                    // Success res from aria2 multicall format: [[ "gid1" ]]
                    if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {
                        let state_clone = state.clone();
                        let gid_clone = returned_gid.to_string();

                        // Fire-and-forget history status update to 'waiting'
						// Mark to `resume` to `waiting`
                        tokio::spawn(async move {
                            HistoryService::handle_event(&state_clone, gid_clone, Some("waiting")).await;
                        });

                        succeeded.push(expected_gid.clone());
                    }
                    // Error response format: { "code": ..., "message": ... }
                    else if let Some(err_obj) = res.as_object() {
                        let borrow_j = &json!(0);
                        let code = err_obj.get("code").unwrap_or(borrow_j);
                        let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");

                        failed.push(json!({
                            "gid": expected_gid,
                            "code": code,
                            "error": msg
                        }));
                    }
                    else {
                        failed.push(json!({
                            "gid": expected_gid,
                            "error": "Aria2 returned invalid response format"
                        }));
                    }
                }
            }

            info!(
                "Batch `{}` executed via multicall: {} succeeded, {} failed",
                method,
                succeeded.len(),
                failed.len()
            );

            //Build response based on outcomes
            if succeeded.is_empty() {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": "Failed to resume specified downloads",
                        "method": method,
                        "failed": failed
                    })),
                ).into_response();
            }

            (
                StatusCode::OK,
                Json(json!({
                    "status": "resumed",
                    "method": method,
                    "succeeded": succeeded,
                    "failed": failed
                })),
            ).into_response()
        }
        Err(e) => {
            error!("`system.multicall` failed for {}: {}", method, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string(), "success": false })),
            ).into_response()
        }
    }
}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.remove
/// It'll remove the ddl from the queue & free the memory
/// Remove only from downloading queue, but don't delete the `file-allocation`
pub async fn stop(
    State(state): State<AppState>,
    Json(payload): Json<StopReq>,
) -> impl IntoResponse {
    // Validate GIDs
    let gids = match &payload.gid {
        Some(gids) if !gids.is_empty() => gids,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "At least one GID is required to stop downloads" })),
            )
                .into_response();
        }
    };

    let method = if payload.force_stop.unwrap_or(false) {
        "aria2.forceRemove"
    } else {
        "aria2.remove"
    };

    // Build system.multicall params
    let calls: Vec<Value> = gids
        .iter()
        .map(|gid| {
            json!({
                "methodName": method,
                "params": [gid]
            })
        })
        .collect();

    // Execute system.multicall
    match state.aria2.call("system.multicall", vec![json!(calls)]).await {
        Ok(multicall_resp) => {
            let mut succeeded = Vec::new();
            let mut failed = Vec::new();

            if let Some(arr) = multicall_resp.as_array() {
                for (i, res) in arr.iter().enumerate() {
                    let expected_gid = &gids[i];

                    // Success response format: [[ "gid1" ]]
                    if let Some(returned_gid) = res.as_array().and_then(|g| g.first()).and_then(|v| v.as_str()) {
                        let state_clone = state.clone();
                        let gid_clone = returned_gid.to_string();

                        // Fire-and-forget: remove download result from aria2 memory & update history DB
                        tokio::spawn(async move {
                            let _ = state_clone.aria2.call("aria2.removeDownloadResult", vec![json!(gid_clone)]).await;
                            HistoryService::handle_event(&state_clone, gid_clone, Some("stopped")).await;
                        });

                        succeeded.push(expected_gid.clone());
                    }
                    // Error response format: { "code": ..., "message": ... }
                    else if let Some(err_obj) = res.as_object() {
                        let borrow_j = &json!(0);
                        let code = err_obj.get("code").unwrap_or(borrow_j);
                        let msg = err_obj.get("message").and_then(|v| v.as_str()).unwrap_or("Unknown error");

                        failed.push(json!({
                            "gid": expected_gid,
                            "code": code,
                            "error": msg
                        }));
                    }
                    else {
                        failed.push(json!({
                            "gid": expected_gid,
                            "error": "Aria2 returned invalid response format"
                        }));
                    }
                }
            }

            info!(
                "Batch `{}` executed via multicall: {} succeeded, {} failed",
                method,
                succeeded.len(),
                failed.len()
            );

            // 4. Build response based on outcomes
            if succeeded.is_empty() {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": "Failed to stop specified downloads",
                        "method": method,
                        "failed": failed
                    })),
                ).into_response();
            }

            (
                StatusCode::OK,
                Json(json!({
                    "status": "stopped",
                    "method": method,
                    "succeeded": succeeded,
                    "failed": failed
                })),
            ).into_response()
        }
        Err(e) => {
            error!("`system.multicall` failed for {}: {}", method, e);
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": e.to_string(), "success": false })),
            ).into_response()
        }
    }
}


/// Get details for a gid
pub async fn details(
    State(state): State<AppState>,
    Json(payload): Json<GidReq>,
) -> impl IntoResponse {
    let gid = payload.gid.clone();
    let history_item = match sqlx::query_as!(
        ItemMetaData,
        r#"
			SELECT
				gid,
				name,
				kind as "kind: Kind",
				status as "status: GidStatus",
				dir,
				files,
				total_length,
				completed_length,
				upload_length,
				source_uri,
				info_hash,
				seeder,
				content_hash,
				error_code,
				error_message,
				torrent as "torrent: sqlx::types::Json<TorrentMagnetMeta>",
				options,
				is_resume_supported,
				connections,
				num_pieces,
				num_seeders,
				piece_length,
				verified_length,
				verify_integrity_pending,
				created_at as "created_at!",
				completed_at as "completed_at",
				user_id
			FROM download_history
			WHERE gid = ?
        "#,
        gid
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(item) => item,
        Err(e) => {
            warn!("Failed to fetch download history for gid {}: {}", gid, e);
            None
        }
    };


    let is_torrent = history_item
        .as_ref()
        .map(|item| matches!(item.kind.clone(), Kind::Torrent)
            || item.torrent.is_some()
            || item.info_hash.is_some())
        .unwrap_or(false);

    let mut methods = vec![
        json!({ "methodName": "aria2.tellStatus", "params": [gid.clone()] }),
        json!({ "methodName": "aria2.getFiles", "params": [gid.clone()] }),
        json!({ "methodName": "aria2.getOption", "params": [gid.clone()] }),
    ];

    if is_torrent {
        methods.push(json!({ "methodName": "aria2.getPeers", "params": [gid.clone()] }));
    } else {
        /*
          * Specific to `uri`
        */
        methods.push(json!({ "methodName": "aria2.getUris", "params": [gid.clone()] }));
        methods.push(json!({ "methodName": "aria2.getServers", "params": [gid.clone()] }));
    }

    let params = vec![json!(methods)];

    match state.aria2.call("system.multicall", params).await {
        Ok(res) => {
            let is_empty = res.as_array().map(|arr| arr.is_empty()).unwrap_or(true);
            if is_empty {
                if let Some(item) = history_item {
                    return (
                        StatusCode::OK,
                        Json(json!({
                            "source": "database",
                            "data": item
                        })),
                    );
                }

                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "error": format!("GID {} not found", gid)
                    })),
                );
            }

            (
                StatusCode::OK,
                Json(json!({
                    "data": {
						"aria2Res": res,
						"history": history_item
					},
                    "source": "aria2",
                })),
            )
        }
        Err(e) => {
            if let Some(item) = history_item {
                warn!("aria2 details lookup failed for gid {:?}: {:?}, Fetching data from db", gid, e);
                (
                    StatusCode::OK,
                    Json(json!({
						"data": item,
                        "source": "database",
                        "warning": e.to_string()
                    })),
                )
            } else {
                error!("Failed to fetch details for gid {:?}: {:?}", gid, e);
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": e.to_string()
                    })),
                )
            }
        }
    }
}

/// Retry a failed download on status `stopped`, `removed`, `error`
/// If server `is_resume_supported`, resume the download
/// If not download again with
/// `allow-overwrite` & `continue=false` for uri
/// `check-integrity=true` for torrents and magnets
pub async fn retry(
    State(state): State<AppState>,
    Json(payload): Json<RetryReq>,
) -> impl IntoResponse {
    let gid = payload.gid.clone();

    let record = sqlx::query!(
        r#"
			SELECT
				dir,
				files,
				options,
				info_hash,
				source_uri,
				total_length,
				is_resume_supported,
				kind as "kind: Kind",
				status as "status: GidStatus",
				torrent as "torrent: sqlx::types::Json<TorrentMagnetMeta>"
			FROM download_history
			WHERE gid = ?
        "#,
        gid
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let rec = match record {
        Some(r) => r,
        None => {
            return (StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("GID {} not found", gid) })))
                .into_response();
        }
    };

    if rec.status != GidStatus::Error
		|| rec.status != GidStatus::Stopped
		|| rec.status != GidStatus::Stopped {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "error": "Only GID with 'stopped', 'removed' and 'error' status can be retried",
                "message": format!("GID {:?} status is {:?}", gid, rec.status)
            })),
        ).into_response();
    }

    let resource_to_add = if let Some(uri) = rec.source_uri.clone() {
        uri
    } else if let Some(hash) = rec.info_hash.as_deref() {
        format!("magnet:?xt=urn:btih:{}", hash)
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Can't retry, no source URI or info hash available" })),
        ).into_response();
    };

    let is_torrent = matches!(rec.kind, Kind::Torrent)
        || rec.torrent.is_some()
        || rec.info_hash.is_some();

    let should_resume = rec.is_resume_supported.unwrap_or(false);

	// clear from aria2 memory
    if !should_resume {
        let _ = state.aria2.call("aria2.forceRemove", vec![json!(gid.clone())]).await;
        let _ = state.aria2.call("aria2.removeDownloadResult", vec![json!(gid.clone())]) .await;
    }

    let mut options = serde_json::Map::new();

    if let Some(dir) = rec.dir.clone() {
        options.insert("dir".to_string(), json!(dir));
    }

    if should_resume {
        options.insert("continue".to_string(), json!(true));
        if is_torrent {
            options.insert("check-integrity".to_string(), json!(true));
        }
    } else {
        options.insert("allow-overwrite".to_string(), json!(true));
        options.insert("continue".to_string(), json!(false));
        options.insert("check-integrity".to_string(), json!(false));
    }

    let params = vec![json!([resource_to_add]), json!(options)];

    match state.aria2.call("aria2.addUri", params).await {
        Ok(new_gid_val) => {
            let new_gid = new_gid_val.as_str().unwrap_or_default();

            let _ = sqlx::query!(
                r#"
					UPDATE download_history
						SET gid = ?,
							error_code = NULL,
							error_message = NULL,
							updated_at = CURRENT_TIMESTAMP
					WHERE gid = ?
                "#,
                new_gid,
                gid
            )
            .execute(&state.db)
            .await;

            (StatusCode::OK, Json(json!({ "status": "success", "newGid": new_gid }))) .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": e })),
        ).into_response(),
    }
}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.saveSession
/// Try to save session
pub async fn save_session(
    State(state): State<AppState>,
) -> impl IntoResponse {

}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeUri
pub async fn change_uri(
    State(state): State<AppState>,
    Json(payload): Json<MoveReq>,
) -> impl IntoResponse {

}

/// Fetches the options snapshot from the local database instead of Aria2,
/// ensuring availability even for `stopped` `removed` downloads.
/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getOption
pub async fn get_option(
    State(state): State<AppState>,
    Json(payload): Json<GidReq>,
) -> impl IntoResponse {
    let row = match sqlx::query!(
        "SELECT options FROM download_history WHERE gid = ?",
        payload.gid
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(row)) => row,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": format!("No download record found for GID '{}'", payload.gid) })),
            );
        }
        Err(e) => {
            error!("Database error fetching options for GID '{}': {:?}", payload.gid, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Internal server database error" })),
            );
        }
    };

    // Options column is present but null, fall back to an empty json
    let options_json: serde_json::Value = match row.options {
        Some(opts_str) if !opts_str.is_empty() => {
            serde_json::from_str(&opts_str).unwrap_or_else(|_| serde_json::json!({}))
        }
        _ => serde_json::json!({}),
    };

    (StatusCode::OK, Json(serde_json::json!({ "data": options_json })))
}

/// Impl for setting options
impl Aria2Options {
    pub fn merge(&mut self, other: Aria2Options) {
        self.0.extend(other.0);
    }

	pub fn to_string_map(&self) -> HashMap<String, String> {
		self.0.iter().map(|(k, v)| {
			let val_str = match v {
				// Extract raw string without quotes
				Value::String(s) => s.clone(),
				// Safely stringify numbers, bools
				_ => v.to_string(),
			};
			(k.clone(), val_str)
		}).collect()
	}
}

/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeOption
pub async fn set_option(
    State(state): State<AppState>,
    Json(payload): Json<SetOptionReq>,
) -> impl IntoResponse {
    let params = vec![
        serde_json::json!(payload.gid),
        serde_json::json!(payload.options),
    ];

    match state.aria2.call("aria2.changeOption", params).await {
        Ok(_) => {
            let existing = sqlx::query!(
                    "SELECT options
                    FROM download_history
                    WHERE gid = ?",
                    payload.gid
                ).fetch_optional(&state.db)
                .await
                .ok()
                .flatten()
                .and_then(|r| r.options)
                .unwrap_or_else(|| "{}".to_string());

            let mut current_opt: Aria2Options
                = serde_json::from_str(&existing).unwrap_or(Aria2Options(serde_json::Map::new()));

            current_opt.merge(payload.options);
            let final_json = serde_json::to_string(&current_opt).unwrap();

            let _ = sqlx::query!(
                "UPDATE download_history
                SET options = ?,
                updated_at = CURRENT_TIMESTAMP
                WHERE gid = ?",
                final_json,
                payload.gid
            ).execute(&state.db).await;

            (StatusCode::OK, Json(serde_json::json!({ "status": "optionsUpdated"})))
        }
        Err(e) => {
            error!("Failed to change options for GID {}: {}", payload.gid, e);
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": e.to_string() })))
        }
    }
}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getGlobalOption
pub async fn get_global_option(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.aria2.call("aria2.getGlobalOption", vec![]).await {
        Ok(options) => (StatusCode::OK,  Json(serde_json::json!({ "data": options }))),
        Err(e) => {
            error!("Failed to fetch global options: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": e.to_string() })))
        }
    }
}


/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.changeGlobalOption
pub async fn set_global_option(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<SetGlobalOptionReq>,
) -> impl IntoResponse {
    let params = vec![serde_json::json!(payload.options)];

    match state.aria2.call("aria2.changeGlobalOption", params).await {
        Ok(_) => {
            let existing = sqlx::query!(
                "SELECT value FROM settings WHERE key = 'aria2_global_options' AND user_id = ?",
                user.user_id
            )
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|r| r.value)
            .unwrap_or_else(|| "{}".to_string());

            let mut current_opt: Aria2Options = serde_json::from_str(&existing)
                .unwrap_or(Aria2Options(serde_json::Map::new()));

            current_opt.merge(payload.options);
            let final_json = serde_json::to_string(&current_opt).unwrap();

            let _ = sqlx::query!(
                r#"
                    INSERT INTO settings (
                        key,
                        value,
                        user_id,
                        updated_at
                    )
                    VALUES (
                        'aria2_global_options',
                        ?,
                        ?,
                        CURRENT_TIMESTAMP
                    )
                    ON CONFLICT(key, user_id) DO UPDATE
                    SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
                "#,
                final_json,
                user.user_id
            )
            .execute(&state.db)
            .await;

            (StatusCode::OK, Json(serde_json::json!({ "status": "globalOptionsUpdated" })))
        },
        Err(e) => {
            error!("Failed to set global options: {}", e);
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "error": e.to_string() })))
        }
    }
}

/// Those options that can't be set via aria2 rpc
pub async fn set_cmd_option(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<SetCmdOptionsReq>,
) -> impl IntoResponse {
    let existing = sqlx::query!(
        "SELECT value FROM settings WHERE key = 'aria2_cmd_options' AND user_id = ?",
        user.user_id
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.value)
    .unwrap_or_else(|| "{}".to_string());

    let mut existing_cmd: HashMap<String, String> = serde_json::from_str(&existing)
		.unwrap_or_default();

    // `Aria2Options` into safe cli strings
    let safe_new_options = payload.options.to_string_map();

    // Delta modifications
    for (key, value) in safe_new_options {
        if value.trim().is_empty() {
            existing_cmd.remove(&key);
        } else {
            existing_cmd.insert(key, value);
        }
    }

    if let Err(err) = state.daemon.restart(&state.config, &existing_cmd).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": format!("Daemon launch failed: {}", err) })),
        );
    }

    // Record mutations safely
    let final_json = serde_json::to_string(&existing_cmd).unwrap();
    let _ = sqlx::query!(
        r#"
            INSERT INTO settings (
				key,
				value,
				user_id,
				updated_at
			)
            VALUES (
				'aria2_cmd_options',
				?,
				?,
				CURRENT_TIMESTAMP
			)
            ON CONFLICT(key, user_id) DO UPDATE
            SET value = excluded.value, updated_at = CURRENT_TIMESTAMP
        "#,
        final_json,
        user.user_id
    )
    .execute(&state.db)
    .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "daemonRestarted",
            "message": "Aria2 daemon restarted with updated arguments."
        })),
    )
}

/*
	Search (no overhead of querying all and ranking)
*/
const SEARCH_DEFAULT_PER_PAGE: u32 = 20;
const SEARCH_MAX_PER_PAGE: u32 = 100;

fn normalize_search_text(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

fn tokenize_search_text(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-')
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn normalized_extension(extension: &str) -> Option<String> {
    let ext = extension.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext.is_empty() {
        None
    } else {
        Some(ext)
    }
}

fn file_type_extensions(file_type: &str) -> Option<&'static [&'static str]> {
    match normalize_search_text(file_type).as_str() {
        "image" | "images" => Some(&[
            "jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "ico", "tif", "tiff", "heic",
            "avif", "raw",
        ]),
        "video" | "videos" => Some(&[
            "mp4", "mkv", "avi", "mov", "webm", "flv", "m4v", "mpg", "mpeg", "ts", "m2ts",
            "wmv", "3gp",
        ]),
        "audio" | "audios" => Some(&[
            "mp3", "flac", "wav", "ogg", "aac", "m4a", "opus", "alac", "ape", "wma",
        ]),
        "document" | "documents" | "doc" | "docs" => Some(&[
            "pdf", "txt", "md", "doc", "docx", "rtf", "epub", "mobi", "djvu", "odt", "ods",
            "xls", "xlsx", "ppt", "pptx", "csv",
        ]),
        "archive" | "archives" => Some(&[
            "zip", "rar", "7z", "tar", "gz", "bz2", "xz", "lz", "lzma", "cab",
        ]),
        "book" | "books" => Some(&[
            "epub", "mobi", "azw", "azw3", "pdf", "cbz", "cbr", "djvu",
        ]),
        "code" | "source" | "sources" => Some(&[
            "rs", "js", "ts", "py", "go", "c", "cpp", "h", "hpp", "java", "sh", "lua", "json",
            "yml", "yaml", "toml", "xml", "html", "css", "sql", "rb", "php", "swift", "kt",
            "kts", "dart", "cs", "md",
        ]),
        "subtitle" | "subtitles" => Some(&["srt", "ass", "ssa", "vtt"]),
        "font" | "fonts" => Some(&["ttf", "otf", "woff", "woff2", "eot"]),
        "torrent" | "torrents" => Some(&["torrent"]),
        "playlist" | "playlists" => Some(&["m3u", "m3u8", "pls"]),
        _ => None,
    }
}

fn push_field_like_predicate(
    qb: &mut sqlx::QueryBuilder<sqlx::Sqlite>,
    column: &str,
    pattern: &str,
) {
    qb.push("LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) LIKE ");
    qb.push_bind(pattern.to_owned());
}

fn push_text_match_predicate(
    qb: &mut sqlx::QueryBuilder<sqlx::Sqlite>,
    q_norm: &str,
    tokens: &[String],
) {
    let prefix = format!("{q_norm}%");
    let contains = format!("%{q_norm}%");

    qb.push("(");
    for (idx, column) in ["name", "source_uri", "files"].iter().enumerate() {
        if idx > 0 {
            qb.push(" OR ");
        }

        qb.push("(");
        push_field_like_predicate(qb, column, &contains);
        qb.push(" OR ");
        push_field_like_predicate(qb, column, &prefix);

        for token in tokens {
            let token_contains = format!("%{token}%");
            qb.push(" OR ");
            push_field_like_predicate(qb, column, &token_contains);
        }

        qb.push(")");
    }
    qb.push(")");
}

fn push_extension_predicate(
    qb: &mut QueryBuilder<Sqlite>,
    extension: &str,
) {
    let Some(ext) = normalized_extension(extension) else {
        qb.push("1 = 0");
        return;
    };

    let pattern = format!("%.{ext}%");

    qb.push("(");
    for (idx, column) in ["name", "source_uri", "files"].iter().enumerate() {
        if idx > 0 {
            qb.push(" OR ");
        }
        push_field_like_predicate(qb, column, &pattern);
    }
    qb.push(")");
}

fn push_category_predicate(
    qb: &mut QueryBuilder<Sqlite>,
    file_type: &str,
) {
    let Some(extensions) = file_type_extensions(file_type) else {
        qb.push("1 = 0");
        return;
    };

    qb.push("(");
    for (i, ext) in extensions.iter().enumerate() {
        if i > 0 {
            qb.push(" OR ");
        }

        let pattern = format!("%.{ext}%");
        qb.push("(");
        for (j, column) in ["name", "source_uri", "files"].iter().enumerate() {
            if j > 0 {
                qb.push(" OR ");
            }
            push_field_like_predicate(qb, column, &pattern);
        }
        qb.push(")");
    }
    qb.push(")");
}

fn push_column_rank_expr(
    qb: &mut QueryBuilder<Sqlite>,
    column: &str,
    q_norm: &str,
    tokens: &[String],
) {
    let exact = q_norm.to_string();
    let prefix = format!("{q_norm}%");
    let contains = format!("%{q_norm}%");

    qb.push("CASE ");
    qb.push("WHEN LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) = ");
    qb.push_bind(exact);
    qb.push(" THEN 120 ");

    qb.push("WHEN LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) LIKE ");
    qb.push_bind(prefix);
    qb.push(" THEN 90 ");

    qb.push("WHEN LOWER(COALESCE(");
    qb.push(column);
    qb.push(", '')) LIKE ");
    qb.push_bind(contains);
    qb.push(" THEN 60 ");

    for token in tokens {
        let token_pattern = format!("%{token}%");
        qb.push("WHEN LOWER(COALESCE(");
        qb.push(column);
        qb.push(", '')) LIKE ");
        qb.push_bind(token_pattern);
        qb.push(" THEN 8 ");
    }

    qb.push("ELSE 0 END");
}

fn push_rank_expr(
    qb: &mut QueryBuilder<Sqlite>,
    q: Option<&str>,
) {
    let Some(q) = q.map(normalize_search_text).filter(|s| !s.is_empty()) else {
        qb.push("0");
        return;
    };

    let tokens = tokenize_search_text(&q);

    qb.push("(");
    push_column_rank_expr(qb, "name", &q, &tokens);
    qb.push(" + ");
    push_column_rank_expr(qb, "source_uri", &q, &tokens);
    qb.push(" + ");
    push_column_rank_expr(qb, "files", &q, &tokens);
    qb.push(")");
}

fn apply_search_filters(
    qb: &mut QueryBuilder<Sqlite>,
    q: Option<&str>,
    file_type: Option<&str>,
    extension: Option<&str>,
) {
    let q = q.map(str::trim).filter(|s| !s.is_empty());
    let file_type = file_type.map(str::trim).filter(|s| !s.is_empty());
    let extension = extension.map(str::trim).filter(|s| !s.is_empty());

    if let Some(q) = q {
        let q_norm = normalize_search_text(q);
        let tokens = tokenize_search_text(&q_norm);

        qb.push(" AND ");
        push_text_match_predicate(qb, &q_norm, &tokens);
    }

    if let Some(file_type) = file_type {
        qb.push(" AND ");
        push_category_predicate(qb, file_type);
    }

    if let Some(extension) = extension {
        qb.push(" AND ");
        push_extension_predicate(qb, extension);
    }
}

fn sanitize_page(page: Option<u32>) -> u32 {
    page.unwrap_or(1).max(1)
}

fn sanitize_per_page(per_page: Option<u32>) -> u32 {
    per_page
        .unwrap_or(SEARCH_DEFAULT_PER_PAGE)
        .clamp(1, SEARCH_MAX_PER_PAGE)
}

/// Search by fuzzy matching name
/// Search by fuzzy source_uri
/// Search by specific file type like (images, video, audio, document)
/// Search by specific file extension like (.mkv, .png, .wav, .pdf)
pub async fn search(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<SearchQuery>,
) -> impl IntoResponse {
    let page = sanitize_page(params.page);
    let per_page = sanitize_per_page(params.per_page);
    let offset = ((page - 1) * per_page) as i64;

    let q = params.q.as_deref();
    let file_type = params.file_type.as_deref();
    let extension = params.extension.as_deref();

    let total: i64 = {
        let mut qb = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM download_history WHERE user_id = ",
        );
        qb.push_bind(&user.user_id);

        apply_search_filters(&mut qb, q, file_type, extension);

        match qb.build_query_scalar().fetch_one(&state.db).await {
            Ok(total) => total,
            Err(e) => {
                error!(
                    "Failed to count search results for user {}: {}",
                    user.user_id, e
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to fetch search results" })),
                );
            }
        }
    };

    let results: Vec<ItemMetaData> = {
        let mut qb = QueryBuilder::<Sqlite>::new("SELECT * FROM download_history WHERE user_id = ");
        qb.push_bind(&user.user_id);

        apply_search_filters(&mut qb, q, file_type, extension);

        qb.push(" ORDER BY ");
        push_rank_expr(&mut qb, q);
        qb.push(" DESC, created_at DESC, gid DESC LIMIT ");
        qb.push_bind(per_page as i64);
        qb.push(" OFFSET ");
        qb.push_bind(offset);

        match qb.build_query_as::<ItemMetaData>().fetch_all(&state.db).await {
            Ok(rows) => rows,
            Err(e) => {
                error!("Failed to fetch search results for user {}: {}", user.user_id, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to fetch search results" })),
                );
            }
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "data": results,
			"meta": {
				"page": page,
				"perPage": per_page,
				"total": total
			}
        })),
    )
}


/// Get `aria2.getVersion`
/// Get supported hash algorithms via command stdout of `aria2 --version`
/// Get libraries info, compiler info
/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getSessionInfo
/// Ref: https://aria2.github.io/manual/en/html/aria2c.html#aria2.getVersion
pub async fn get_aria2_info(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> impl IntoResponse {
    let rpc_version = match state.aria2.call("aria2.getVersion", vec![]).await {
        Ok(version) => version,
        Err(e) => {
            error!("Failed to get aria2 version: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": e.to_string()
                })),
            );
        }
    };

    let session_id = match state.aria2.call("aria2.getSessionInfo", vec![]).await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to get aria2 session id: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": e.to_string()
                })),
            );
        }
    };

    let cli_info = match Command::new("aria2c").arg("--version").output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parsed = parse_aria2c_version_stdout(&stdout);

            if output.status.success() {
                parsed
            } else {
                json!({
                    "error": "aria2c --version exited with a non-zero status",
                    "stdout": parsed
                })
            }
        }
        Err(e) => {
            warn!("Failed to execute `aria2c --version`: {}", e);
            json!({
                "error": e.to_string()
            })
        }
    };

    let mut data_obj = json!({});
	if let Some(obj) = cli_info.as_object() {
        for (k, v) in obj {
            data_obj[k] = v.clone();
        }
    }
    if let Some(obj) = rpc_version.as_object() {
        for (k, v) in obj {
            data_obj[k] = v.clone();
        }
    }
    if let Some(obj) = session_id.as_object() {
        for (k, v) in obj {
            data_obj[k] = v.clone();
        }
    }

    (
        StatusCode::OK,
		Json(json!({
			"data": data_obj
		})),
    )
}

fn parse_aria2c_version_stdout(stdout: &str) -> Value {
    let mut hash_algorithms: Vec<String> = Vec::new();
    let mut libraries: Vec<Value> = Vec::new();
    let mut compiler = serde_json::Map::new();
    let mut in_compiler_section = false;

    for raw_line in stdout.lines() {
        let line = raw_line.trim_end();

        if let Some(rest) = line.strip_prefix("Hash Algorithms:") {
            hash_algorithms = rest
                .split(',')
                .map(|item| item.trim())
                .filter(|item| !item.is_empty())
                .map(|item| item.to_string())
                .collect();
            continue;
        }

        if let Some(rest) = line.strip_prefix("Libraries:") {
            libraries = rest
                .split_whitespace()
                .filter_map(|item| {
                    let (name, version) = item.split_once('/')?;
                    Some(json!({
                        "name": name,
                        "version": version
                    }))
                })
                .collect();
            continue;
        }

        if let Some(rest) = line.strip_prefix("Compiler:") {
            let rest = rest.trim();
            let mut parts = rest.split_whitespace();
            if let Some(name) = parts.next() {
                compiler.insert("name".to_string(), json!(name));
            }

            let version = parts.collect::<Vec<_>>().join(" ");
            if !version.is_empty() {
                compiler.insert("version".to_string(), json!(version));
            }

            in_compiler_section = true;
            continue;
        }

        if in_compiler_section {
            let trimmed = line.trim_start();

            if let Some(rest) = trimmed.strip_prefix("built by") {
                compiler.insert("builtBy".to_string(), json!(rest.trim()));
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("on") {
                compiler.insert("builtOn".to_string(), json!(rest.trim()));
                continue;
            }

            if let Some(rest) = trimmed.strip_prefix("System:") {
                compiler.insert("system".to_string(), json!(rest.trim()));
                continue;
            }

            if trimmed.is_empty()
                || trimmed.starts_with("Report bugs to")
                || trimmed.starts_with("Visit ")
            {
                in_compiler_section = false;
            }
        }
    }

    json!({
        "hashAlgorithms": hash_algorithms,
        "libraries": libraries,
        "compiler": compiler
    })
}


pub async fn purge_results(State(state): State<AppState>) -> impl IntoResponse {
    match state.aria2.call("aria2.purgeDownloadResult", vec![]).await {
        Ok(_) => (StatusCode::OK, Json(json!({ "status": "purged" }))),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": e }))),
    }
}


pub async fn move_position(
    State(state): State<AppState>,
    Json(payload): Json<MoveReq>,
) -> impl IntoResponse {
    let params = vec![
        json!(payload.gid),
        json!(payload.pos),
        json!(payload.how)
    ];

    state.aria2.call("aria2.changePosition", params)
        .await
        .inspect(|res| info!("Successfully executed multicall: {:?}", res))
        .inspect_err(|e| info!("failed to add uri: {:?}", e))
        .map(|results| (StatusCode::OK, Json(json!({ "newPosition": results }))))
        .unwrap_or_else(|e| (StatusCode::BAD_GATEWAY, Json(json!({ "error": e.to_string() }))))
}

/// History sort impls
impl SortBy {
    pub fn to_sql_column(&self) -> &'static str {
        match self {
            SortBy::Size => "total_length",
            SortBy::Name => "name",
            SortBy::Time => "created_at",
        }
    }
}

impl Order {
    pub fn to_sql_keyword(&self) -> &'static str {
        match self {
            Order::Asc => "ASC",
            Order::Desc => "DESC",
        }
    }
}

/// Get dl history based on query
pub async fn get_history(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Query(params): Query<PaginationQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(15);
    let offset = (page - 1) * limit;

	// Resolve column name direction from enums
    let order_column = params
        .sort_by
        .map(|s| s.to_sql_column())
        .unwrap_or("created_at");

    let order_direction = params
        .order
        .map(|o| o.to_sql_keyword())
        .unwrap_or("DESC");

    let total_items = sqlx::query!(
        "SELECT COUNT(*) as count FROM download_history WHERE user_id = ?",
        user.user_id
    )
    .fetch_one(&state.db)
    .await
    .map(|row| row.count)
    .unwrap_or(0);

    let total_pages = if total_items == 0 {
        0
    } else {
        (total_items as f64 / limit as f64).ceil() as u32
    };

    let has_more = page < total_pages;

	let mut query: sqlx::QueryBuilder<sqlx::Sqlite> = sqlx::QueryBuilder::new(
        r#"
        SELECT
            gid,
			name,
			kind,
			status,
			dir,
			files,
			total_length,
			completed_length,
            upload_length,
			source_uri,
			info_hash,
			seeder,
			content_hash,
			error_code,
            error_message,
			torrent,
			options,
			is_resume_supported,
			connections,
            num_pieces,
			num_seeders,
			piece_length,
			verified_length,
            verify_integrity_pending,
			created_at,
			completed_at,
			user_id
        FROM download_history
        WHERE user_id = "#
    );
	query.push_bind(user.user_id);

    // Dynamically append the sort params
    query.push(" ORDER BY ");
    query.push(order_column);
    query.push(" ");
    query.push(order_direction);
    query.push(" LIMIT ");
    query.push_bind(limit);
    query.push(" OFFSET ");
    query.push_bind(offset);

	let history: Vec<ItemMetaData> = query
        .build_query_as::<ItemMetaData>()
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    Json(json!({
        "data": history,
        "meta": {
            "currentPage": page,
            "perPage": limit,
            "totalItems": total_items,
            "totalPages": total_pages,
            "hasMore": has_more
        }
    }))
}


/// Remove dl along with deleting file
pub async fn delete(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<RemoveHisReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    for gid in payload.gids {
        /*
          * First free from aria2
          * Kinda inefficient, later check the status then do according to it
        */
        let _ = state.aria2.call("aria2.forcePause", vec![json!(gid)]).await;
        let _ = state.aria2.call("aria2.forceRemove", vec![json!(gid)]).await;
        let _ = state.aria2.call("aria2.removeDownloadResult", vec![json!(gid)]).await;

        if payload.delete_file {
            let record = sqlx::query!(
                "SELECT name, files, dir FROM download_history
                WHERE gid = ? AND user_id = ?",
                gid, user.user_id
            )
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

            debug!("Found gid to delete: {:?}", record);

            if let Some(rec) = record {
                let download_dir = rec.dir.as_deref().map(Path::new);
                if let Some(name) = rec.name.as_ref() {
                    let name = name.to_string();
                    if let Some(files_str) = rec.files {
                        let file_paths: Vec<String> = serde_json::from_str(&files_str).unwrap_or_default();
                        for path in file_paths {
                            let path = Path::new(&path);
                            debug!("Removing file: {:?}", path);
                            if !path.exists() {
                                continue;
                            }
                            if path.is_dir() {
                                let _ = std::fs::remove_dir_all(&path);
                            } else {
                                let _ = std::fs::remove_file(&path);
                            }
                            if let Some(root) = download_dir {
                                let aria_name = format!("{}.aria2", name);
                                let aria2_path = root.join(Path::new(&aria_name));
                                let _ = std::fs::remove_file(aria2_path);
                                let mut current = path.parent();
                                while let Some(dir) = current {
                                    if dir == root {
                                        break;
                                    }

                                    if !dir.exists() || !dir.is_dir() {
                                        break;
                                    }

                                    let is_empty = std::fs::read_dir(dir)
                                        .map(|mut entries| entries.next().is_none())
                                        .unwrap_or(false);

                                    if !is_empty {
                                        break;
                                    }

                                    let _ = std::fs::remove_dir(dir);
                                    current = dir.parent();
                                }
                            }
                        }
                    }
                } else {
                    debug!("Skipping file deletion for gid {:?}: name is NULL", gid);
                }
            } else {
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("No download history found from GID {}", gid) })),
                ));
            }
        }

        /* Ensure `user_id` matches history */
        let res = sqlx::query!(
            "DELETE FROM download_history WHERE gid = ? AND user_id = ?",
            gid, user.user_id
        )
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))))?;

        if res.rows_affected() == 0 {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": format!("Download history record not found from GID {}", gid)
                })),
            ));
        }

        info!("Deleted gid: {:?}", gid);
    }

    Ok(Json(json!({ "status": "deleted" })))
}

